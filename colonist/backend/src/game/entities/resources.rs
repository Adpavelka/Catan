use std::collections::HashMap;

use rand::Rng;
use serde::{Deserialize, Serialize};
pub(crate) use shared::ResourceType;
use shared::ResourceType::{Brick, Ore, Sheep, Wheat, Wood};
use shared::Resources;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceSet {
    amounts: HashMap<ResourceType, u32>,
}

impl ResourceSet {
    pub fn new() -> Self {
        Self {
            amounts: HashMap::new(),
        }
    }

    pub fn amount_of(&self, res: ResourceType) -> u32 {
        *self.amounts.get(&res).unwrap_or(&0)
    }

    pub fn can_pay(&self, cost: &ResourceSet) -> bool {
        for (res, amount) in &cost.amounts {
            if self.amount_of(*res) < *amount {
                return false;
            }
        }
        true
    }

    /// Removes up to `amount`. Saturating rather than wrapping: an unchecked
    /// subtraction here panics in debug and silently mints ~4 billion cards in
    /// release. Callers are still expected to `can_pay` first.
    pub fn take(&mut self, res: ResourceType, amount: u32) {
        let entry = self.amounts.entry(res).or_insert(0);
        if amount > *entry {
            log::warn!(
                "Tried to take {} {:?} but only {} held; clamping to 0",
                amount,
                res,
                entry
            );
        }
        *entry = entry.saturating_sub(amount);
    }

    // TODO, private ne?
    pub fn add(&mut self, res: ResourceType, amount: u32) {
        if res == ResourceType::Desert {
            return;
        }
        *self.amounts.entry(res).or_insert(0) += amount;
    }

    pub fn add_set(&mut self, set: &ResourceSet) {
        for (res, amount) in &set.amounts {
            self.add(*res, *amount);
        }
    }

    pub fn take_set(&mut self, set: &ResourceSet) {
        for (res, &amt) in &set.amounts {
            self.take(*res, amt);
        }
    }

    // TODO, move to player
    pub fn get_cards_total(&self) -> u32 {
        self.amounts.values().sum()
    }

    // TODO, move to player
    pub fn take_random_card(&mut self) -> Option<ResourceType> {
        let total = self.get_cards_total();
        if total == 0 {
            return None;
        }

        let mut rng = rand::thread_rng();
        let mut choice = rng.gen_range(0..total);

        for (resource, amount) in self.amounts.iter_mut() {
            if *amount == 0 {
                continue;
            }

            if choice < *amount {
                *amount -= 1;
                return Some(*resource);
            } else {
                choice -= *amount;
            }
        }

        None
    }
}

impl From<&ResourceSet> for Resources {
    fn from(r: &ResourceSet) -> Self {
        Resources {
            brick: *r.amounts.get(&Brick).unwrap_or(&0) as u8,
            lumber: *r.amounts.get(&Wood).unwrap_or(&0) as u8,
            wool: *r.amounts.get(&Sheep).unwrap_or(&0) as u8,
            grain: *r.amounts.get(&Wheat).unwrap_or(&0) as u8,
            ore: *r.amounts.get(&Ore).unwrap_or(&0) as u8,
        }
    }
}




#[cfg(test)]
mod tests {
    use crate::game::entities::resources::{ResourceType, ResourceSet};
    #[test]
    fn resource_set_new_starts_empty() {
        let rs = ResourceSet::new();
        assert_eq!(rs.amount_of(ResourceType::Wood), 0);
        assert_eq!(rs.amount_of(ResourceType::Brick), 0);
        assert_eq!(rs.amount_of(ResourceType::Sheep), 0);
        assert_eq!(rs.amount_of(ResourceType::Wheat), 0);
        assert_eq!(rs.amount_of(ResourceType::Ore), 0);
        assert_eq!(rs.amount_of(ResourceType::Desert), 0);
    }

    #[test]
    fn add_increases_amount_and_desert_is_ignored() {
        let mut rs = ResourceSet::new();
        rs.add(ResourceType::Wood, 2);
        rs.add(ResourceType::Wood, 3);
        rs.add(ResourceType::Desert, 100);

        assert_eq!(rs.amount_of(ResourceType::Wood), 5);
        assert_eq!(rs.amount_of(ResourceType::Desert), 0, "desert should never be stored");
    }

    #[test]
    fn take_decreases_amount() {
        let mut rs = ResourceSet::new();
        rs.add(ResourceType::Brick, 4);
        rs.take(ResourceType::Brick, 2);
        assert_eq!(rs.amount_of(ResourceType::Brick), 2);
    }

    /// Overdrawing must not wrap around into a huge balance.
    #[test]
    fn take_more_than_held_clamps_to_zero() {
        let mut rs = ResourceSet::new();
        rs.add(ResourceType::Brick, 2);

        rs.take(ResourceType::Brick, 5);

        assert_eq!(rs.amount_of(ResourceType::Brick), 0);
        assert_eq!(rs.get_cards_total(), 0);
    }

    #[test]
    fn get_cards_total_sums_all_resources() {
        let mut rs = ResourceSet::new();
        rs.add(ResourceType::Wood, 2);
        rs.add(ResourceType::Brick, 1);
        rs.add(ResourceType::Sheep, 3);
        rs.add(ResourceType::Wheat, 4);
        rs.add(ResourceType::Ore, 5);

        assert_eq!(rs.get_cards_total(), 15);
    }

    #[test]
    fn can_pay_true_when_sufficient_and_false_when_insufficient() {
        let mut have = ResourceSet::new();
        have.add(ResourceType::Wood, 2);
        have.add(ResourceType::Brick, 1);

        let mut cost_ok = ResourceSet::new();
        cost_ok.add(ResourceType::Wood, 2);
        cost_ok.add(ResourceType::Brick, 1);

        let mut cost_too_much = ResourceSet::new();
        cost_too_much.add(ResourceType::Wood, 3);

        let mut cost_missing_type = ResourceSet::new();
        cost_missing_type.add(ResourceType::Ore, 1);

        assert!(have.can_pay(&cost_ok));
        assert!(!have.can_pay(&cost_too_much));
        assert!(!have.can_pay(&cost_missing_type));
    }

    #[test]
    fn take_random_card_returns_none_when_empty() {
        let mut rs = ResourceSet::new();
        assert_eq!(rs.take_random_card(), None);
    }

    #[test]
    fn take_random_card_reduces_total_by_one_and_returns_existing_resource() {
        let mut rs = ResourceSet::new();
        rs.add(ResourceType::Wood, 2);
        rs.add(ResourceType::Ore, 1);

        let total_before = rs.get_cards_total();
        let wood_before = rs.amount_of(ResourceType::Wood);
        let ore_before = rs.amount_of(ResourceType::Ore);

        let taken = rs.take_random_card().expect("should take a card");
        let total_after = rs.get_cards_total();

        assert_eq!(total_after, total_before - 1);

        match taken {
            ResourceType::Wood => {
                assert_eq!(rs.amount_of(ResourceType::Wood), wood_before - 1);
                assert_eq!(rs.amount_of(ResourceType::Ore), ore_before);
            }
            ResourceType::Ore => {
                assert_eq!(rs.amount_of(ResourceType::Ore), ore_before - 1);
                assert_eq!(rs.amount_of(ResourceType::Wood), wood_before);
            }
            other => panic!("unexpected resource taken: {:?}", other),
        }
    }
}
