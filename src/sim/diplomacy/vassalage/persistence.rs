//! Validation for saved pending vassal submission clauses.
use super::*;

impl Simulation {
    pub(in crate::sim) fn validate_vassal_offers(&self) -> Result<(), &'static str> {
        for (&pair, relation) in &self.diplomacy {
            let Some(offer) = relation.offer.as_ref() else {
                continue;
            };
            let Some(terms) = offer.vassalage else {
                continue;
            };
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
            if offer.pact.is_some()
                || offer.terms != [Concessions::default(), Concessions::default()]
            {
                return Err("Invalid saved vassal offer clauses");
            }
        }
        Ok(())
    }
}
