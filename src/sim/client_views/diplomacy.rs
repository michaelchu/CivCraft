//! Diplomacy, treaty, and city deal projections.

use super::*;

impl Simulation {
    pub(crate) fn diplomacy_meeting_view(
        &self,
        owner: usize,
        draft: Option<TreatyOffer>,
    ) -> Option<DiplomacyMeetingView> {
        if owner == HUMAN {
            return None;
        }
        let leader = self.players.get(owner)?;
        let pact = self.pact(HUMAN, owner);
        let ai_offer = self.treaty_offer(owner);
        let next_pact = self.next_pact(HUMAN, owner);
        let offer = draft.or(ai_offer).unwrap_or(TreatyOffer {
            pact: next_pact,
            ..TreatyOffer::default()
        });
        let after = offer.pact.unwrap_or(pact);
        let sides = [HUMAN, owner].map(|side| {
            let other = if side == HUMAN { owner } else { HUMAN };
            TreatySideView {
                name: self.players[side].name.clone(),
                gold: self.players[side].gold,
                technologies: self
                    .treaty_trade_technologies(side, other)
                    .into_iter()
                    .map(|tech| (tech, tech.name().to_owned()))
                    .collect(),
                can_offer_embassy: self.treaty_can_offer_embassy(side, other),
                can_offer_vision: self.treaty_can_offer_vision(side, other, after),
                cities: self
                    .treaty_trade_cities(side, after)
                    .into_iter()
                    .filter_map(|id| self.city_by_id(id).map(|city| (id, city.name.clone())))
                    .collect(),
            }
        });
        Some(DiplomacyMeetingView {
            leader_line: format!("{} · {}", leader.name, leader.leader_name()),
            can_negotiate: self.can_negotiate(owner),
            ai_offer,
            pact,
            next_pact,
            offer,
            sides,
        })
    }

    pub(crate) fn treaty_offer_current_view(&self, owner: usize, offer: TreatyOffer) -> bool {
        self.treaty_offer(owner) == Some(offer)
    }

    pub(crate) fn diplomacy_overview_view(&self) -> Vec<DiplomacyRivalView> {
        (1..self.players.len())
            .filter_map(|owner| {
                let can_negotiate = self.can_negotiate(owner);
                let treaty_offer = self.treaty_offer(owner);
                if !can_negotiate && treaty_offer.is_none() {
                    return None;
                }
                let pact = self.pact(HUMAN, owner);
                let player = &self.players[owner];
                Some(DiplomacyRivalView {
                    owner,
                    name: player.name.clone(),
                    leader_name: player.leader_name().to_owned(),
                    pact,
                    pact_turns: self.pact_turns(HUMAN, owner),
                    has_embassy: self.has_embassy(HUMAN, owner),
                    senate_veto: pact != Pact::War && self.senate_blocks_cancellation(HUMAN, owner),
                    pact_offer: self.pact_offer(owner),
                    next_pact: self.next_pact(HUMAN, owner),
                    can_negotiate,
                    treaty_offer,
                    world_map_price: self.map_trade_price(owner, HUMAN, TreatyMap::World, pact),
                    sea_map_price: self.map_trade_price(owner, HUMAN, TreatyMap::Sea, pact),
                    has_city_deals: self
                        .cities
                        .iter()
                        .any(|city| city.owner == owner && self.city_trade_price(city).is_some()),
                })
            })
            .collect()
    }

    pub(crate) fn city_deals_view(&self, owner: usize) -> Vec<CityDealView> {
        self.cities
            .iter()
            .filter(|city| city.owner == owner)
            .filter_map(|city| {
                let price = self.city_trade_price(city)?;
                Some(CityDealView {
                    id: city.id,
                    name: city.name.clone(),
                    price,
                    affordable: self.players[HUMAN].gold >= price,
                })
            })
            .collect()
    }

    pub(crate) fn treasury_view(&self, owner: usize) -> Option<i32> {
        self.players.get(owner).map(|player| player.gold)
    }

    pub(crate) fn player_name_view(&self, owner: usize) -> Option<String> {
        self.players.get(owner).map(|player| player.name.clone())
    }

    pub(crate) fn city_name_view(&self, city_id: u32) -> Option<String> {
        self.city_by_id(city_id).map(|city| city.name.clone())
    }
}
