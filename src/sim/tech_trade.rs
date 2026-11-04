use super::*;

impl Simulation {
    fn technology_gold_equivalent(&self, buyer: usize, tech: Tech) -> i32 {
        // Civ2Civ3 disables trades that would leave holes in the tech tree.
        if tech == Tech::FUTURE || !self.tech_available(buyer, tech) {
            return 0;
        }
        let bulbs: i32 = self
            .research_path(buyer, tech)
            .into_iter()
            .map(|required| self.target_cost(buyer, required))
            .sum();
        let want = self.advisor_tech_want(buyer, tech).max(0) / self.turn.max(1);
        // With no holes allowed, all eligible buyers already know the prerequisites.
        (3 * bulbs + want) / 2
    }

    pub(super) fn tech_sell_price(&self, seller: usize, buyer: usize, tech: Tech) -> Option<i32> {
        if !self.has_tech(seller, tech) || tech == Tech::FUTURE || !self.tech_available(buyer, tech)
        {
            return None;
        }
        let mut price = self.technology_gold_equivalent(buyer, tech);
        if self.allied(seller, buyer) {
            price /= 2;
        }
        for third in 0..self.players.len() {
            if third == seller
                || third == buyer
                || third == BARBARIANS
                || !self.player_alive(third)
                || self.has_tech(third, tech)
                || !self.allied(buyer, third)
            {
                continue;
            }
            if self.tech_trade_partner_is_dangerous(seller, third) {
                return None;
            }
            if !self.allied(seller, third) {
                price += self.technology_gold_equivalent(third, tech) / 4;
            }
        }
        Some(price)
    }

    pub(super) fn ai_accepts_technology_exchange(
        &self,
        a: usize,
        b: usize,
        terms: &[super::diplomacy::Concessions; 2],
    ) -> bool {
        if terms.iter().all(|term| term.technology.is_none()) {
            return true;
        }
        for (side, (owner, other)) in [(a, b), (b, a)].into_iter().enumerate() {
            let player = &self.players[owner];
            if !player.ai {
                continue;
            }
            let give = terms[side];
            let receive = terms[1 - side];
            let mut balance = receive.gold - give.gold;
            if let Some(tech) = give.technology {
                let Some(price) = self.tech_sell_price(owner, other, tech) else {
                    return false;
                };
                balance -= price;
            }
            if let Some(tech) = receive.technology {
                let Some(mut price) = self.tech_sell_price(other, owner, tech) else {
                    return false;
                };
                let researched = player.known.len() as i32 + player.future_techs + 1;
                let minimum = (self.technology_upkeep(owner) / researched).max(1);
                if player.last_science < minimum {
                    if player.research < 0 {
                        return false;
                    }
                    price /= 2;
                }
                balance += price;
            }
            if balance < 0 {
                return false;
            }
        }
        true
    }
}
