//! City-bar timing mirrors common/city.c's turns_to_build/turns_to_grow.
use super::*;

pub(super) fn label_top(center_y: f32, zoom: f32) -> f32 {
    // Keep a stable anchor with or without focus: the complete tile-sized
    // selection sprite extends farther down than either the unit or city.
    center_y + TILE_H * 0.5 * zoom + (4.0 * zoom).max(3.0)
}

fn build_turns(stock: i32, cost: i32, surplus: i32) -> Option<i32> {
    if stock >= cost {
        Some(1)
    } else if surplus > 0 {
        Some((cost - stock - 1) / surplus + 1)
    } else {
        None
    }
}
fn growth_text(food: i32, capacity: i32, surplus: i32, can_grow: bool) -> String {
    if food + surplus >= capacity && !can_grow {
        "Growth blocked".into()
    } else if food + surplus >= capacity {
        "Growth 1".into()
    } else if surplus > 0 {
        format!("Growth {}", (capacity - food + surplus - 1) / surplus)
    } else if surplus < 0 {
        format!("Famine {}", 1 - food / surplus)
    } else {
        "Growth -".into()
    }
}

impl Game {
    pub(super) fn city_bar_detail(&self, city: &City) -> Option<String> {
        // Production and food stocks are private city information.
        if city.owner != HUMAN {
            return None;
        }
        let forecast = self.sim.city_forecast(city);
        let growth = growth_text(
            city.food,
            Simulation::growth_box(city.size),
            forecast.food_surplus(city.size, forecast.support.food),
            city.size < Simulation::city_size_limit(city),
        );
        let production = match city.production {
            Production::Building(b) if b.data().genus == Genus::Convert => {
                city.production.name().to_owned()
            }
            _ => {
                // Native timing only treats an already-claimed great wonder
                // as unavailable. Other build prerequisites do not gate this
                // stock/surplus estimate for the current production target.
                let available = match city.production {
                    Production::Building(b) if b.data().genus == Genus::GreatWonder => {
                        self.sim.great_wonder_available(b)
                    }
                    _ => true,
                };
                let turns = available
                    .then(|| {
                        build_turns(
                            city.shields,
                            city.production.cost(),
                            forecast.shield_surplus(),
                        )
                    })
                    .flatten()
                    .filter(|turns| *turns <= 999);
                format!(
                    "{} {}",
                    city.production.name(),
                    turns.map_or_else(|| "-".into(), |t| t.to_string())
                )
            }
        };
        Some(format!("{production} · {growth}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn city_label_clears_the_selection_footprint_at_every_supported_zoom() {
        for zoom in [MIN_ZOOM, START_ZOOM, 1.0, MAX_ZOOM, 1.5] {
            assert!(label_top(100.0, zoom) >= 100.0 + TILE_H * zoom * 0.5 + 3.0);
        }
    }
    #[test]
    fn production_and_growth_follow_native_stock_and_surplus_boundaries() {
        assert_eq!(build_turns(30, 30, 0), Some(1));
        assert_eq!(build_turns(0, 30, 0), None);
        assert_eq!(build_turns(1, 30, 10), Some(3));
        assert_eq!(growth_text(0, 20, 3, true), "Growth 7");
        assert_eq!(growth_text(18, 20, 2, false), "Growth blocked");
        assert_eq!(growth_text(18, 20, 2, true), "Growth 1");
        assert_eq!(growth_text(4, 20, -2, true), "Famine 3");
        assert_eq!(growth_text(0, 20, 0, true), "Growth -");
    }
}
