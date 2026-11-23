//! Translation from Civ III world choices to Freeciv fractal parameters.
//!
//! The campaign exposes Civ III's coarse world settings. The fractal generator
//! consumes Freeciv's native 0-100 parameters and subdivision count, so the
//! translation happens once, when a generation request is built, and the
//! generator itself sees only Freeciv-native values.

use super::defaults::{
    self, MapGenerationSettings, StartPositionScoringSettings, TerrainMapgenProperties,
};
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

/// Freeciv ruleset data consumed only by the fractal generator.
pub(crate) struct FractalRuleset {
    pub(crate) map_generation_settings: MapGenerationSettings,
    pub(crate) terrain_properties: Vec<TerrainMapgenProperties>,
    pub(crate) start_scoring_settings: StartPositionScoringSettings,
}

impl Default for FractalRuleset {
    fn default() -> Self {
        Self {
            map_generation_settings: defaults::default_map_generation_settings(),
            terrain_properties: defaults::default_terrain_mapgen_properties(),
            start_scoring_settings: defaults::default_start_position_scoring_settings(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> MapGenerationSettings {
        defaults::default_map_generation_settings()
    }

    fn resolve(world: WorldSettings) -> FractalParameters {
        FractalParameters::resolve(base(), world, 8)
    }

    #[test]
    fn default_world_keeps_the_ruleset_values_except_landmass() {
        let world = WorldSettings::default();
        let resolved = resolve(world);
        let expected = MapGenerationSettings {
            landmass_percent: 100 - world.ocean_coverage.percent(),
            ..base()
        };
        assert_eq!(resolved.settings, expected);
        assert_eq!(resolved.divisions, base().startpos.fractal_divisions(8));
    }

    #[test]
    fn climate_choices_set_wetness() {
        let wetness = |climate| {
            resolve(WorldSettings {
                climate,
                ..WorldSettings::default()
            })
            .settings
            .wetness
        };
        assert_eq!(wetness(Climate::Arid), 25);
        assert_eq!(wetness(Climate::Normal), base().wetness);
        assert_eq!(wetness(Climate::Wet), 75);
    }

    #[test]
    fn temperature_choices_set_temperature() {
        let temperature = |temperature| {
            resolve(WorldSettings {
                temperature,
                ..WorldSettings::default()
            })
            .settings
            .temperature
        };
        assert_eq!(temperature(Temperature::Cool), 40);
        assert_eq!(temperature(Temperature::Temperate), base().temperature);
        assert_eq!(temperature(Temperature::Warm), 80);
    }

    #[test]
    fn age_choices_set_steepness() {
        let steepness = |age| {
            resolve(WorldSettings {
                age,
                ..WorldSettings::default()
            })
            .settings
            .steepness
        };
        assert_eq!(steepness(WorldAge::Billion3), 45);
        assert_eq!(steepness(WorldAge::Billion4), base().steepness);
        assert_eq!(steepness(WorldAge::Billion5), 15);
    }

    #[test]
    fn landform_choices_set_subdivisions() {
        let divisions = |landform| {
            resolve(WorldSettings {
                landform,
                ..WorldSettings::default()
            })
            .divisions
        };
        let default_divisions = base().startpos.fractal_divisions(8);
        assert_eq!(divisions(Landform::Pangaea), 2);
        assert_eq!(divisions(Landform::Continents), default_divisions);
        assert_eq!(divisions(Landform::Archipelago), default_divisions + 2);
    }

    #[test]
    fn default_ruleset_uses_the_campaign_defaults() {
        let ruleset = FractalRuleset::default();
        assert_eq!(ruleset.map_generation_settings, base());
        assert_eq!(
            ruleset.start_scoring_settings,
            defaults::default_start_position_scoring_settings()
        );
        assert_eq!(
            ruleset.terrain_properties.len(),
            defaults::default_terrain_mapgen_properties().len()
        );
    }
}
