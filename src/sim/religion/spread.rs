//! Simultaneous influence resolution from a frozen source snapshot.
use super::*;
impl Simulation {
    pub(in crate::sim) fn advance_religion(&mut self) {
        let r = rules();
        self.religion
            .cities
            .retain(|id, _| self.cities.iter().any(|c| c.id == *id));
        self.run_religion_ai();
        let snapshot = self.religion.cities.clone();
        let by_id: BTreeMap<_, _> = self.cities.iter().map(|c| (c.id, c)).collect();
        // Tile buckets bound domestic candidate searches to a small neighborhood.
        let mut positions: BTreeMap<(i32, i32), Vec<u32>> = BTreeMap::new();
        for c in &self.cities {
            positions.entry((c.x, c.y)).or_default().push(c.id);
        }
        let mut routes: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        for (a, b) in self.active_trade_city_pairs() {
            if let (Some(x), Some(y)) = (by_id.get(&a), by_id.get(&b))
                && (x.owner == y.owner
                    || self
                        .contacted_pact(x.owner, y.owner)
                        .is_some_and(|p| p != Pact::War))
            {
                routes.entry(a).or_default().insert(b);
                routes.entry(b).or_default().insert(a);
            }
        }
        let mut incoming: BTreeMap<(u32, ReligionId), BTreeMap<u32, i32>> = BTreeMap::new();
        for (&source, s) in &snapshot {
            let Some(faith) = s.dominant else { continue };
            let Some(c) = by_id.get(&source) else {
                continue;
            };
            let nation = self.national_faith(c.owner);
            let bonus =
                if nation.policy == FaithPolicy::Missionary && nation.declared == Some(faith) {
                    r.missionary_pressure
                } else {
                    0
                };
            let intrinsic = definition(faith).unwrap();
            let mut targets = BTreeMap::new();
            // Civ III native tiles: bound a square, then use the topology's exact distance.
            for dy in -2 * r.local_distance..=2 * r.local_distance {
                for dx in -2 * r.local_distance..=2 * r.local_distance {
                    let x = (c.x + dx).rem_euclid(self.map_wrap.width());
                    let y = c.y + dy;
                    if let Some(ids) = positions.get(&(x, y)) {
                        for id in ids {
                            let t = by_id[id];
                            if *id != source
                                && t.owner == c.owner
                                && self.map_wrap.civ3_distance(c.x, c.y, t.x, t.y)
                                    <= r.local_distance
                            {
                                targets.insert(*id, r.local_pressure + intrinsic.outgoing_pressure);
                            }
                        }
                    }
                }
            }
            if let Some(ids) = routes.get(&source) {
                for id in ids {
                    targets
                        .entry(*id)
                        .and_modify(|v| {
                            *v = (*v).max(
                                r.trade_pressure
                                    + intrinsic.outgoing_pressure
                                    + intrinsic.trade_pressure,
                            )
                        })
                        .or_insert(
                            r.trade_pressure
                                + intrinsic.outgoing_pressure
                                + intrinsic.trade_pressure,
                        );
                }
            }
            for (target, pressure) in targets {
                incoming
                    .entry((target, faith))
                    .or_default()
                    .insert(source, pressure + bonus);
            }
        }
        let cities: Vec<_> = self
            .cities
            .iter()
            .map(|c| (c.id, c.owner, c.name.clone()))
            .collect();
        for (city, owner, name) in cities {
            let old = snapshot.get(&city).cloned().unwrap_or_default();
            let mut next = old.clone();
            for value in next.influence.values_mut() {
                *value = (*value - (*value + r.decay_divisor - 1) / r.decay_divisor).max(0);
            }
            for f in &self.religion.religions {
                let mut add = if f.holy_city == city {
                    r.holy_pressure
                } else {
                    0
                };
                if old.dominant == Some(f.id) {
                    add += r.community_pressure + definition(f.id).unwrap().local_pressure;
                    let n = self.national_faith(owner);
                    if n.policy == FaithPolicy::Devout && n.declared == Some(f.id) {
                        add += r.devout_pressure;
                    }
                }
                add += incoming.get(&(city, f.id)).map_or(0, |sources| {
                    sources.values().sum::<i32>().min(r.incoming_cap)
                });
                if add > 0 {
                    let value = next.influence.entry(f.id).or_default();
                    *value = (*value + add).min(r.maximum_influence);
                }
            }
            next.influence.retain(|_, v| *v > 0);
            next.dominant = dominant(&next);
            let total = r.unaligned_weight + next.influence.values().sum::<i32>();
            let present: Vec<_> = next
                .influence
                .iter()
                .filter(|(_, v)| **v * 100 >= total * r.presence_percent)
                .map(|(&id, _)| id)
                .collect();
            let mut changes = Vec::new();
            if old.dominant != next.dominant {
                changes.push(("converted", next.dominant));
            }
            for id in present {
                if next.announced.insert(id) && next.dominant != Some(id) {
                    changes.push(("present", Some(id)));
                }
            }
            self.religion.cities.insert(city, next);
            for (action, faith) in changes {
                self.religion_event(action, owner, Some(city), faith, None);
                let source = faith
                    .and_then(|id| incoming.get(&(city, id)))
                    .and_then(|s| {
                        s.iter()
                            .max_by_key(|(id, v)| (**v, std::cmp::Reverse(**id)))
                    })
                    .map(|(&id, _)| id);
                let own_source = source
                    .and_then(|id| self.city_by_id(id))
                    .filter(|c| c.owner == HUMAN);
                // Foreign city attribution is only public while the city is visible.
                let visible = self
                    .city_by_id(city)
                    .is_some_and(|c| self.tile(c.x, c.y).visible);
                if owner == HUMAN || (own_source.is_some() && visible) {
                    let label = faith
                        .map(|id| self.religion_name(id).to_owned())
                        .unwrap_or_else(|| "mixed beliefs".into());
                    let via = own_source.map_or(String::new(), |c| {
                        format!(" with influence from {}", c.name)
                    });
                    let message = if action == "converted" {
                        format!("{name} has adopted {label}{via}.")
                    } else {
                        format!("{label} has reached {name}{via}.")
                    };
                    self.add_contextual_notification(
                        message,
                        NotificationCategory::Religion,
                        if owner == HUMAN && action == "converted" {
                            NotificationPriority::Important
                        } else {
                            NotificationPriority::Routine
                        },
                        Some(city),
                        Some(owner),
                    );
                }
            }
        }
    }
}
