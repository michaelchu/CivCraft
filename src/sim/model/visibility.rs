//! Packed exploration flags. The save boundary retains its existing byte encoding.
use crate::sim::PLAYER_SLOTS;
use serde::{Deserializer, Serializer, de};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct KnownBy(pub(crate) [u8; PLAYER_SLOTS.div_ceil(8)]);

impl serde::Serialize for KnownBy {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> serde::Deserialize<'de> for KnownBy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visibility;

        impl de::Visitor<'_> for Visibility {
            type Value = KnownBy;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("packed campaign visibility bytes")
            }

            fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
                bytes
                    .try_into()
                    .map(KnownBy)
                    .map_err(|_| E::invalid_length(bytes.len(), &self))
            }
        }

        deserializer.deserialize_bytes(Visibility)
    }
}

impl KnownBy {
    pub(crate) fn set(&mut self, player: usize, known: bool) {
        assert!(player < PLAYER_SLOTS);
        let mask = 1 << (player % 8);
        if known {
            self.0[player / 8] |= mask;
        } else {
            self.0[player / 8] &= !mask;
        }
    }

    pub(crate) fn fill(&mut self, known: bool) {
        self.0.fill(if known { u8::MAX } else { 0 });
    }
}

impl std::ops::Index<usize> for KnownBy {
    type Output = bool;
    fn index(&self, player: usize) -> &bool {
        assert!(player < PLAYER_SLOTS);
        if self.0[player / 8] & (1 << (player % 8)) != 0 {
            &true
        } else {
            &false
        }
    }
}

impl From<[bool; PLAYER_SLOTS]> for KnownBy {
    fn from(flags: [bool; PLAYER_SLOTS]) -> Self {
        let mut known = Self([0; PLAYER_SLOTS.div_ceil(8)]);
        for (player, flag) in flags.into_iter().enumerate() {
            known.set(player, flag);
        }
        known
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flags_preserve_every_player_and_byte_boundary() {
        assert_eq!(std::mem::size_of::<KnownBy>(), 64);
        let mut flags = KnownBy([0; PLAYER_SLOTS.div_ceil(8)]);
        for player in 0..PLAYER_SLOTS {
            flags.set(player, true);
            for other in 0..PLAYER_SLOTS {
                assert_eq!(flags[other], other <= player);
            }
        }
        for player in 0..PLAYER_SLOTS {
            flags.set(player, false);
            for other in 0..PLAYER_SLOTS {
                assert_eq!(flags[other], other > player);
            }
        }
        flags.fill(true);
        assert!((0..PLAYER_SLOTS).all(|player| flags[player]));
        flags.fill(false);
        assert!((0..PLAYER_SLOTS).all(|player| !flags[player]));
        assert!(std::mem::size_of::<crate::sim::Tile>() < 200);
    }

    #[test]
    #[should_panic]
    fn rejects_out_of_range_player() {
        let _ = KnownBy([0; PLAYER_SLOTS.div_ceil(8)])[PLAYER_SLOTS];
    }
}
