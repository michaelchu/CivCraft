//! World options shared by the desktop and headless entry points.

use crate::mapgen::{self, FractalMapGenerator, MapSize, OpenSimplexMapGenerator, WorldSettings};
use crate::world::GeneratedMap;
use serde_json::{Value, json};

const GENERATOR_FLAG: &str = "--map-generator";
const SIZE_FLAG: &str = "--map-size";
const OCEAN_FLAG: &str = "--ocean-coverage";
const CLIMATE_FLAG: &str = "--climate";
const AGE_FLAG: &str = "--world-age";
const LANDFORM_FLAG: &str = "--landform";
const TEMPERATURE_FLAG: &str = "--temperature";
pub(crate) const FLAGS: &[&str] = &[
    SIZE_FLAG,
    GENERATOR_FLAG,
    OCEAN_FLAG,
    CLIMATE_FLAG,
    AGE_FLAG,
    LANDFORM_FLAG,
    TEMPERATURE_FLAG,
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Generator {
    #[default]
    Fractal,
    OpenSimplex,
}

impl Generator {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fractal => "fractal",
            Self::OpenSimplex => "opensimplex",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorldSetup {
    pub(crate) size: MapSize,
    pub(crate) generator: Generator,
    pub(crate) settings: WorldSettings,
    pub(crate) specified: bool,
}

impl WorldSetup {
    pub(crate) fn settings_metadata(self) -> Value {
        json!({
            "ocean_coverage": self.settings.ocean_coverage.percent(),
            "climate": format!("{:?}", self.settings.climate).to_ascii_lowercase(),
            "world_age_billions": self.settings.age.billions(),
            "landform": format!("{:?}", self.settings.landform).to_ascii_lowercase(),
            "temperature": format!("{:?}", self.settings.temperature).to_ascii_lowercase()
        })
    }

    pub(crate) fn parse(
        default_size: MapSize,
        mut value: impl FnMut(&str) -> Result<Option<String>, String>,
    ) -> Result<Self, String> {
        let size = value(SIZE_FLAG)?;
        let generator = value(GENERATOR_FLAG)?;
        let ocean = value(OCEAN_FLAG)?;
        let climate = value(CLIMATE_FLAG)?;
        let age = value(AGE_FLAG)?;
        let landform = value(LANDFORM_FLAG)?;
        let temperature = value(TEMPERATURE_FLAG)?;
        let specified = size.is_some()
            || generator.is_some()
            || ocean.is_some()
            || climate.is_some()
            || age.is_some()
            || landform.is_some()
            || temperature.is_some();
        let generator = match generator.as_deref() {
            None | Some("fractal") => Generator::Fractal,
            Some("opensimplex") => Generator::OpenSimplex,
            Some(name) => {
                return Err(format!(
                    "invalid {GENERATOR_FLAG} value {name:?}; expected fractal or opensimplex"
                ));
            }
        };
        let default = WorldSettings::default();
        let settings = WorldSettings {
            ocean_coverage: parse_option(ocean, OCEAN_FLAG)?.unwrap_or(default.ocean_coverage),
            climate: parse_option(climate, CLIMATE_FLAG)?.unwrap_or(default.climate),
            age: parse_option(age, AGE_FLAG)?.unwrap_or(default.age),
            landform: parse_option(landform, LANDFORM_FLAG)?.unwrap_or(default.landform),
            temperature: parse_option(temperature, TEMPERATURE_FLAG)?
                .unwrap_or(default.temperature),
        };
        Ok(Self {
            size: parse_option(size, SIZE_FLAG)?.unwrap_or(default_size),
            generator,
            settings,
            specified,
        })
    }

    pub(crate) fn generate(self, seed: u64) -> GeneratedMap {
        match self.generator {
            Generator::Fractal => mapgen::generate_for_map_size_with(
                &FractalMapGenerator,
                seed,
                self.size,
                self.settings,
            ),
            Generator::OpenSimplex => mapgen::generate_for_map_size_with(
                &OpenSimplexMapGenerator,
                seed,
                self.size,
                self.settings,
            ),
        }
    }
}

fn parse_option<T: std::str::FromStr<Err = String>>(
    value: Option<String>,
    flag: &str,
) -> Result<Option<T>, String> {
    value
        .map(|value| value.parse())
        .transpose()
        .map_err(|error| format!("invalid {flag} value: {error}"))
}
