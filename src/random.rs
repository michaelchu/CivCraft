//! Freeciv utility/rand.c's gameplay generator, independent of map generation.
//! Keep the entire state: restoring a seed cannot resume an ongoing game.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SavedState", into = "SavedState")]
pub(crate) struct FreecivRng {
    values: Vec<u32>,
    j: u8,
    k: u8,
    x: u8,
    initialized: bool,
}

#[derive(Serialize, Deserialize)]
struct SavedState {
    values: Vec<u32>,
    j: u8,
    k: u8,
    x: u8,
    initialized: bool,
}

impl From<FreecivRng> for SavedState {
    fn from(state: FreecivRng) -> Self {
        Self {
            values: state.values,
            j: state.j,
            k: state.k,
            x: state.x,
            initialized: state.initialized,
        }
    }
}

impl TryFrom<SavedState> for FreecivRng {
    type Error = &'static str;
    fn try_from(state: SavedState) -> Result<Self, Self::Error> {
        let result = Self {
            values: state.values,
            j: state.j,
            k: state.k,
            x: state.x,
            initialized: state.initialized,
        };
        if result.valid() {
            Ok(result)
        } else {
            Err("invalid Freeciv random state")
        }
    }
}

impl Default for FreecivRng {
    fn default() -> Self {
        Self {
            values: vec![0; 56],
            j: 0,
            k: 0,
            x: 0,
            initialized: false,
        }
    }
}

impl FreecivRng {
    /// Convert the equivalent map generator cursor without reheating or drawing.
    pub(crate) fn from_map_state(values: [u32; 56], next: usize) -> Result<Self, &'static str> {
        if next >= 56 {
            return Err("invalid Freeciv map random cursor");
        }
        let state = Self {
            values: values.to_vec(),
            j: next as u8,
            k: ((next + 31) % 56) as u8,
            x: ((next + 55) % 56) as u8,
            initialized: true,
        };
        debug_assert!(state.valid());
        Ok(state)
    }

    pub(crate) fn new(seed: u32) -> Self {
        let mut state = Self::default();
        state.values[0] = seed;
        for i in 1..56 {
            state.values[i] = state.values[i - 1].wrapping_mul(3).wrapping_add(257);
        }
        state.j = 0;
        state.k = 31;
        state.x = 55;
        state.initialized = true;
        for _ in 0..10_000 {
            state.draw(u32::MAX);
        }
        state
    }

    pub(crate) fn valid(&self) -> bool {
        self.values.len() == 56
            && self.j < 56
            && self.k < 56
            && self.x < 56
            && (!self.initialized || ((self.x + 1) % 56 == self.j && (self.j + 31) % 56 == self.k))
    }

    pub(crate) fn is_initialized(&self) -> bool {
        self.initialized
    }

    #[cfg(test)]
    pub(crate) fn uninitialize(&mut self) {
        self.initialized = false;
    }

    /// fc_rand_debug accepts an unsigned size, including zero. Sizes zero
    /// and one return zero without advancing; rejection consumes extra draws.
    pub(crate) fn draw(&mut self, size: u32) -> u32 {
        if !self.initialized || size <= 1 {
            return 0;
        }
        let divisor = u32::MAX / size;
        let maximum = size * divisor - 1;
        let mut attempts = 0;
        loop {
            let value = self.values[self.j as usize].wrapping_add(self.values[self.k as usize]);
            self.x = (self.x + 1) % 56;
            self.j = (self.j + 1) % 56;
            self.k = (self.k + 1) % 56;
            self.values[self.x as usize] = value;
            attempts += 1;
            // Native bailout happens after writing and advancing the 10001st
            // attempt, even when that last value would have been accepted.
            if attempts > 10_000 {
                return 0;
            }
            if value <= maximum {
                return value / divisor;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_hash(state: &FreecivRng) -> u32 {
        state
            .values
            .iter()
            .copied()
            .chain([
                u32::from(state.j),
                u32::from(state.k),
                u32::from(state.x),
                u32::from(state.initialized),
            ])
            .fold(2_166_136_261, |hash, word| {
                (hash ^ word).wrapping_mul(16_777_619)
            })
    }

    #[test]
    fn native_c_oracle_matches_values_and_entire_state_after_rejections() {
        let mut current_seed = None;
        let mut state = FreecivRng::default();
        for line in include_str!("../tests/fixtures/freeciv_random_vectors.tsv").lines() {
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<u32> = line.split('\t').map(|s| s.parse().unwrap()).collect();
            if current_seed != Some(fields[0]) {
                state = FreecivRng::new(fields[0]);
                current_seed = Some(fields[0]);
            }
            assert_eq!(state.draw(fields[1]), fields[2], "{line}");
            assert_eq!(state_hash(&state), fields[3], "{line}");
            assert!(state.valid());
        }
    }

    #[test]
    fn small_limits_uninitialized_and_save_resume_preserve_draw_order() {
        let mut state = FreecivRng::default();
        assert!(!state.is_initialized());
        assert_eq!(state.draw(100), 0);
        let mut state = FreecivRng::new(42);
        let saved = state.clone();
        assert_eq!(state.draw(0), 0);
        assert_eq!(state.draw(1), 0);
        assert_eq!(state, saved);
        for _ in 0..120 {
            state.draw(100);
        }
        let bytes = bincode::serialize(&state).unwrap();
        let mut restored: FreecivRng = bincode::deserialize(&bytes).unwrap();
        for limit in [u32::MAX, 2, 2_147_483_649, 100, 7] {
            assert_eq!(state.draw(limit), restored.draw(limit));
        }
        assert_eq!(state, restored);
        state.uninitialize();
        let before = state.clone();
        assert_eq!(state.draw(99), 0);
        assert_eq!(state, before);
    }

    #[test]
    fn loading_rejects_truncated_words_invalid_indices_and_inconsistent_phase() {
        for mutation in 0..4 {
            let mut saved = SavedState::from(FreecivRng::new(7));
            match mutation {
                0 => {
                    saved.values.pop();
                }
                1 => saved.j = 56,
                2 => saved.k = (saved.k + 1) % 56,
                _ => saved.x = 255,
            }
            let bytes = bincode::serialize(&saved).unwrap();
            assert!(bincode::deserialize::<FreecivRng>(&bytes).is_err());
        }
    }

    #[test]
    fn rejected_maximum_advances_before_drawing_again() {
        let mut state = FreecivRng {
            values: vec![0; 56],
            j: 0,
            k: 31,
            x: 55,
            initialized: true,
        };
        // MAX is excluded for a size-MAX draw: divisor one, maximum MAX-1.
        state.values[0] = u32::MAX;
        assert_eq!(state.draw(u32::MAX), 0);
        assert_eq!((state.j, state.k, state.x), (2, 33, 1));
        assert_eq!(state.values[0], u32::MAX);
        assert!(state.valid());
    }
}
