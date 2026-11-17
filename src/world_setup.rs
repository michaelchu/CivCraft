//! World options shared by the desktop and headless entry points.

use crate::mapgen::{self, Civ3MapSize, Civ3WorldSettings};
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

#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum Generator {
    #[default]
    Fractal,
    OpenSimplex(Civ3WorldSettings),
}

impl Generator {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Fractal => "fractal",
            Self::OpenSimplex(_) => "opensimplex",
        }
    }

    pub(crate) fn settings_metadata(self) -> Value {
        match self {
            Self::Fractal => json!(mapgen::campaign_map_generation_settings()),
            Self::OpenSimplex(settings) => json!({
                "ocean_coverage": settings.ocean_coverage.percent(),
                "climate": format!("{:?}", settings.climate).to_ascii_lowercase(),
                "world_age_billions": settings.age.billions(),
                "landform": format!("{:?}", settings.landform).to_ascii_lowercase(),
                "temperature": format!("{:?}", settings.temperature).to_ascii_lowercase()
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WorldSetup {
    pub(crate) size: Civ3MapSize,
    pub(crate) generator: Generator,
    pub(crate) specified: bool,
}

impl WorldSetup {
    pub(crate) fn parse(
        default_size: Civ3MapSize,
        mut value: impl FnMut(&str) -> Result<Option<String>, String>,
    ) -> Result<Self, String> {
        let size = value(SIZE_FLAG)?;
        let generator = value(GENERATOR_FLAG)?;
        let ocean = value(OCEAN_FLAG)?;
        let climate = value(CLIMATE_FLAG)?;
        let age = value(AGE_FLAG)?;
        let landform = value(LANDFORM_FLAG)?;
        let temperature = value(TEMPERATURE_FLAG)?;
        let custom = [
            (OCEAN_FLAG, &ocean),
            (CLIMATE_FLAG, &climate),
            (AGE_FLAG, &age),
            (LANDFORM_FLAG, &landform),
            (TEMPERATURE_FLAG, &temperature),
        ];
        let explicit_control = custom.iter().find(|(_, option)| option.is_some());
        let specified = size.is_some() || generator.is_some() || explicit_control.is_some();
        let generator = match generator.as_deref() {
            None | Some("fractal") => {
                if let Some((flag, _)) = explicit_control {
                    return Err(format!("{flag} requires {GENERATOR_FLAG} opensimplex"));
                }
                Generator::Fractal
            }
            Some("opensimplex") => {
                let default = Civ3WorldSettings::default();
                Generator::OpenSimplex(Civ3WorldSettings {
                    ocean_coverage: parse_option(ocean, OCEAN_FLAG)?
                        .unwrap_or(default.ocean_coverage),
                    climate: parse_option(climate, CLIMATE_FLAG)?.unwrap_or(default.climate),
                    age: parse_option(age, AGE_FLAG)?.unwrap_or(default.age),
                    landform: parse_option(landform, LANDFORM_FLAG)?.unwrap_or(default.landform),
                    temperature: parse_option(temperature, TEMPERATURE_FLAG)?
                        .unwrap_or(default.temperature),
                })
            }
            Some(name) => {
                return Err(format!(
                    "invalid {GENERATOR_FLAG} value {name:?}; expected fractal or opensimplex"
                ));
            }
        };
        Ok(Self {
            size: parse_option(size, SIZE_FLAG)?.unwrap_or(default_size),
            generator,
            specified,
        })
    }

    pub(crate) fn generate(self, seed: u64) -> GeneratedMap {
        match self.generator {
            Generator::Fractal => mapgen::generate_for_civ3_size(seed, self.size),
            Generator::OpenSimplex(settings) => {
                mapgen::generate_opensimplex_for_civ3_size(seed, self.size, settings)
            }
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
