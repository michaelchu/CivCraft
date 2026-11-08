//! Route construction and traversal helpers for the parent subsystem.

use super::*;

impl Simulation {
    pub(super) fn weighted_trade_distance(&self, a: &City, b: &City) -> i32 {
        self.map_distance(a.x, a.y, b.x, b.y) * 40
            / self.map_wrap.width().max(self.map_wrap.height())
    }

    pub(super) fn same_continent(&self, a: &City, b: &City) -> bool {
        let mut seen = vec![false; self.tiles.len()];
        let mut queue = vec![(a.x, a.y)];
        seen[self.map_wrap.index(a.x, a.y)] = true;
        while let Some((x, y)) = queue.pop() {
            if (x, y) == (b.x, b.y) {
                return true;
            }
            for (nx, ny) in self.map_wrap.civ3_neighbors(x, y) {
                let index = self.map_wrap.index(nx, ny);
                if !seen[index] && !self.tiles[index].terrain.is_water() {
                    seen[index] = true;
                    queue.push((nx, ny));
                }
            }
        }
        false
    }

    pub(in crate::sim) fn route_value(&self, a: &City, b: &City) -> i32 {
        let value = self.weighted_trade_distance(a, b) + a.size + b.size;
        value * if self.same_continent(a, b) { 1 } else { 2 } / 12
    }

    fn route_partners(&self, city: u32) -> impl Iterator<Item = &City> {
        self.trade_routes.iter().filter_map(move |&(a, b)| {
            let partner = if a == city {
                b
            } else if b == city {
                a
            } else {
                return None;
            };
            self.city_by_id(partner)
        })
    }

    pub(in crate::sim) fn route_trade(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .map(|other| self.route_value(city, other))
            .sum()
    }

    pub(in crate::sim) fn trade_illness(&self, city: &City) -> i32 {
        self.route_partners(city.id)
            .filter(|other| self.can_trade(city, other))
            .filter(|other| other.last_plague.is_some_and(|turn| self.turn - turn < 5))
            .map(|other| 0.5 * f64::from(city.size * other.size).sqrt())
            .sum::<f64>() as i32
    }

    pub(super) fn route_to_replace(&self, city: &City, value: i32) -> Result<Option<u32>, ()> {
        let partners: Vec<_> = self.route_partners(city.id).collect();
        if partners.len() < 2 {
            return Ok(None);
        }
        let weakest = partners
            .iter()
            .map(|other| (self.route_value(city, other), other.id))
            .min();
        match weakest {
            Some((old, id)) if old < value => Ok(Some(id)),
            _ => Err(()),
        }
    }

    pub(super) fn remove_route(&mut self, a: u32, b: u32) {
        self.trade_routes
            .retain(|&pair| pair != (a, b) && pair != (b, a));
    }

    pub(in crate::sim) fn prune_trade_routes(&mut self) {
        let mut routes = std::mem::take(&mut self.trade_routes);
        routes.retain(|&(a, b)| {
            let from = self.city_by_id(a);
            let to = self.city_by_id(b);
            from.zip(to)
                .is_some_and(|(from, to)| self.can_trade(from, to))
        });
        self.trade_routes = routes;
    }

    pub(super) fn route_replacements(
        &self,
        from: &City,
        to: &City,
    ) -> Option<(Option<u32>, Option<u32>)> {
        if self
            .route_partners(from.id)
            .any(|partner| partner.id == to.id)
        {
            return None;
        }
        let value = self.route_value(from, to);
        self.route_to_replace(from, value)
            .ok()
            .zip(self.route_to_replace(to, value).ok())
    }
}
