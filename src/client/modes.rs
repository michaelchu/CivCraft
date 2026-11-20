//! Mutually exclusive interaction states of the game screen.
//!
//! The client is always in exactly one targeting mode and shows at most one
//! full-screen overlay. Modelling each as an enum makes entering a state
//! replace the previous one, so no handler has to reset its siblings.

/// What the next map click means for the selected unit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum TargetingMode {
    /// A map click selects whatever is on the tile.
    #[default]
    None,
    Goto,
    Connect,
    Patrol,
    Paradrop,
    CaptureUnits,
    Bombard {
        /// Target a city improvement rather than a tile.
        precision: bool,
        /// Repeat the bombardment on following turns.
        auto: bool,
    },
    NuclearStrike,
}

impl TargetingMode {
    /// The mode, or [`Self::None`] when the selected unit cannot use it.
    pub(super) fn when(self, available: bool) -> Self {
        if available { self } else { Self::None }
    }

    /// The instruction shown over the map while the mode waits for a target.
    /// Connect and patrol are announced through their action buttons only.
    pub(super) fn banner(self) -> Option<&'static str> {
        Some(match self {
            Self::None | Self::Connect | Self::Patrol => return None,
            Self::Goto => "GO-TO: choose a destination",
            Self::Paradrop => "PARADROP: choose a landing tile",
            Self::CaptureUnits => "CAPTURE UNITS: choose an adjacent enemy stack",
            Self::Bombard {
                precision: true, ..
            } => "PRECISION BOMBING: choose an enemy city",
            Self::Bombard { .. } => "BOMBARD: choose an enemy tile",
            Self::NuclearStrike => "NUCLEAR STRIKE: choose a target",
        })
    }
}

/// The full-screen view drawn instead of, or over, the world map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Overlay {
    #[default]
    None,
    Intro,
    Research,
    Spaceship,
    Government,
    Religion,
    Diplomacy,
    Espionage,
}

impl Overlay {
    /// Opens `target`, or closes it when it is already the active overlay.
    pub(super) fn toggle(&mut self, target: Self) {
        *self = if *self == target { Self::None } else { target };
    }

    /// Closes `target` if it is the active overlay; other overlays stay open.
    pub(super) fn close(&mut self, target: Self) {
        if *self == target {
            *self = Self::None;
        }
    }

    /// Overlays that replace the world map entirely.
    pub(super) fn hides_map(self) -> bool {
        matches!(self, Self::Research | Self::Spaceship)
    }

    /// Overlays drawn as a dimmed panel above the world map.
    pub(super) fn is_council(self) -> bool {
        matches!(
            self,
            Self::Government | Self::Religion | Self::Diplomacy | Self::Espionage
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_targeting_falls_back_to_selection() {
        assert_eq!(TargetingMode::Paradrop.when(false), TargetingMode::None);
        assert_eq!(TargetingMode::Paradrop.when(true), TargetingMode::Paradrop);
    }

    #[test]
    fn only_modes_with_a_map_prompt_show_a_banner() {
        for quiet in [
            TargetingMode::None,
            TargetingMode::Connect,
            TargetingMode::Patrol,
        ] {
            assert_eq!(quiet.banner(), None);
        }
        let bombard = |precision| TargetingMode::Bombard {
            precision,
            auto: false,
        };
        assert_ne!(bombard(true).banner(), bombard(false).banner());
        assert!(TargetingMode::Goto.banner().is_some());
    }

    #[test]
    fn toggling_an_overlay_replaces_any_other_and_closes_itself() {
        let mut overlay = Overlay::Government;
        overlay.toggle(Overlay::Espionage);
        assert_eq!(overlay, Overlay::Espionage);
        overlay.close(Overlay::Spaceship);
        assert_eq!(overlay, Overlay::Espionage);
        overlay.toggle(Overlay::Espionage);
        assert_eq!(overlay, Overlay::None);
    }
}
