//! Save extension for agreements, schedules and pending submission clauses.
use super::*;

/// Extension to the frozen campaign core, including clauses omitted by legacy offers.
#[derive(Serialize, Deserialize)]
pub(in crate::sim) struct SavedVassalage {
    state: VassalState,
    offers: Vec<((usize, usize), VassalTerms)>,
}
impl Simulation {
    pub(in crate::sim) fn save_vassalage(&self) -> SavedVassalage {
        SavedVassalage {
            state: self.vassalage.clone(),
            offers: self
                .diplomacy
                .iter()
                .filter_map(|(&pair, r)| r.offer.as_ref()?.vassalage.map(|t| (pair, t)))
                .collect(),
        }
    }
    pub(in crate::sim) fn restore_vassalage(
        &mut self,
        saved: SavedVassalage,
    ) -> Result<(), &'static str> {
        self.vassalage = saved.state;
        for (pair, terms) in saved.offers {
            if pair.0 >= pair.1
                || pair.1 >= self.civilization_count
                || pair.1 >= self.players.len()
                || ![pair.0, pair.1].contains(&terms.master)
            {
                return Err("Invalid saved vassal offer participants");
            }
            let subject = if terms.master == pair.0 {
                pair.1
            } else {
                pair.0
            };
            if !self.players[subject].ai || [pair.0, pair.1].contains(&BARBARIANS) {
                return Err("Invalid saved vassal offer subject");
            }
            let offer = self
                .diplomacy
                .get_mut(&pair)
                .and_then(|r| r.offer.as_mut())
                .ok_or("Vassal clause has no offer")?;
            if offer.vassalage.replace(terms).is_some()
                || offer.pact.is_some()
                || offer.terms != [Concessions::default(), Concessions::default()]
            {
                return Err("Invalid saved vassal offer clauses");
            }
        }
        Ok(())
    }
}
