//! Translation from Civ III world choices to Freeciv fractal parameters.
//!
//! The campaign exposes Civ III's coarse world settings. The fractal generator
//! consumes Freeciv's native 0-100 parameters and subdivision count, so the
//! translation happens once, when a generation request is built, and the
//! generator itself sees only Freeciv-native values.

use super::defaults::MapGenerationSettings;
use super::{Climate, Landform, Temperature, WorldAge, WorldSettings};

/// Freeciv fractal inputs resolved from [`WorldSettings`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FractalParameters {
    pub(crate) settings: MapGenerationSettings,
    pub(crate) divisions: i32,
}

impl FractalParameters {
    /// Overlays `world` on the ruleset `base`; `Normal`/`Temperate`/4-billion
    /// choices keep the base value.
    pub(crate) fn resolve(
        mut base: MapGenerationSettings,
        world: WorldSettings,
        civilization_count: usize,
    ) -> Self {
        base.landmass_percent = 100 - world.ocean_coverage.percent();
        base.wetness = match world.climate {
            Climate::Arid => 25,
            Climate::Normal => base.wetness,
            Climate::Wet => 75,
        };
        base.temperature = match world.temperature {
            Temperature::Cool => 40,
            Temperature::Temperate => base.temperature,
            Temperature::Warm => 80,
        };
        base.steepness = match world.age {
            WorldAge::Billion3 => 45,
            WorldAge::Billion4 => base.steepness,
            WorldAge::Billion5 => 15,
        };
        let default_divisions = base.startpos.fractal_divisions(civilization_count);
        let divisions = match world.landform {
            Landform::Pangaea => 2,
            Landform::Continents => default_divisions,
            Landform::Archipelago => default_divisions + 2,
        };
        Self {
            settings: base,
            divisions,
        }
    }
}
