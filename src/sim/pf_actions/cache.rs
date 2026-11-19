//! Pure movement facts keyed by complete tile, class and policy descriptors.
//! Rule data is immutable; a changed tile or relationship produces a different key.
use crate::sim::{
    movement_rules::{MovementRules, MovementTile},
    unit_catalog::UnitClass,
};
use std::{cell::RefCell, collections::HashMap};

/// Derived movement data lives with the code that constructs its cache keys.
#[derive(Clone, Default)]
pub(in crate::sim) struct MovementCache {
    step_cost: RefCell<HashMap<u32, i32>>,
    native: RefCell<Vec<i8>>,
}

impl MovementCache {
    pub(super) fn native_to_class(&self, tile: MovementTile, class: UnitClass) -> bool {
        let index = ((class as usize) << MovementTile::CACHE_BITS) | tile.cache_index() as usize;
        let cache_len = (UnitClass::Missile as usize + 1) << MovementTile::CACHE_BITS;
        {
            let mut cache = self.native.borrow_mut();
            if cache.is_empty() {
                cache.resize(cache_len, -1);
            }
            if cache[index] >= 0 {
                return cache[index] != 0;
            }
        }
        let native = MovementRules::campaign()
            .movement_tile_native_to_class(
                tile.terrain,
                tile.road,
                tile.railroad,
                tile.river,
                tile.base,
                class,
            )
            .expect("campaign C2C3 movement native_to data");
        self.native.borrow_mut()[index] = i8::from(native);
        native
    }
    pub(super) fn step_cost(
        &self,
        source_movement: MovementTile,
        destination_movement: MovementTile,
        class: UnitClass,
        cardinal: bool,
        restricted: bool,
        ignores: bool,
    ) -> i32 {
        const CLASS_BITS: u32 = 3;
        const FLAG_BITS: u32 = 3;
        const { assert!((UnitClass::Missile as u32) < 1 << CLASS_BITS) };
        let tiles = (source_movement.cache_index() << MovementTile::CACHE_BITS)
            | destination_movement.cache_index();
        let key = (tiles << (CLASS_BITS + FLAG_BITS))
            | ((class as u32) << FLAG_BITS)
            | (u32::from(cardinal) << 2)
            | (u32::from(restricted) << 1)
            | u32::from(ignores);
        if let Some(cost) = self.step_cost.borrow().get(&key).copied() {
            return cost;
        }
        let cost = MovementRules::campaign()
            .step_cost(
                source_movement,
                destination_movement,
                class,
                cardinal,
                restricted,
                ignores,
            )
            .expect("campaign C2C3 step movement data");
        self.step_cost.borrow_mut().insert(key, cost);
        cost
    }
}
