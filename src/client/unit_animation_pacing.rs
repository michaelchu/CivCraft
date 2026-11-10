//! Editable presentation pacing layered over each unit INI's source timings.

use crate::sim::UnitKind;
use serde::Deserialize;
use std::collections::HashMap;

const PACING_TOML: &str =
    include_str!("../../data/rulesets/civ3_conquests/unit_animation_pacing.toml");

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct FidgetPacing {
    pub(super) play_seconds: f32,
    pub(super) rest_seconds: f32,
}

impl Default for FidgetPacing {
    fn default() -> Self {
        Self {
            play_seconds: 1.5,
            rest_seconds: 6.5,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct PacingDefaults {
    work_cycle_duration_multiplier: f32,
    fidget_play_seconds: f32,
    fidget_rest_seconds: f32,
}

impl Default for PacingDefaults {
    fn default() -> Self {
        Self {
            work_cycle_duration_multiplier: 1.0,
            fidget_play_seconds: 1.5,
            fidget_rest_seconds: 6.5,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ActionPacing {
    cycle_duration_multiplier: f32,
}

impl Default for ActionPacing {
    fn default() -> Self {
        Self {
            cycle_duration_multiplier: 1.0,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct UnitPacingOverrides {
    work_cycle_duration_multiplier: Option<f32>,
    fidget: Option<FidgetPacing>,
    actions: HashMap<String, ActionPacing>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(super) struct UnitAnimationPacing {
    defaults: PacingDefaults,
    units: HashMap<String, UnitPacingOverrides>,
}

impl Default for UnitAnimationPacing {
    fn default() -> Self {
        Self {
            defaults: PacingDefaults::default(),
            units: HashMap::new(),
        }
    }
}

impl UnitAnimationPacing {
    pub(super) fn load() -> Result<Self, String> {
        let pacing: Self = toml::from_str(PACING_TOML)
            .map_err(|error| format!("invalid unit animation pacing TOML: {error}"))?;
        pacing.validate()?;
        Ok(pacing)
    }

    pub(super) fn fidget(&self, kind: UnitKind) -> FidgetPacing {
        self.unit_overrides(kind)
            .and_then(|overrides| overrides.fidget)
            .unwrap_or(FidgetPacing {
                play_seconds: self.defaults.fidget_play_seconds,
                rest_seconds: self.defaults.fidget_rest_seconds,
            })
    }

    pub(super) fn cycle_duration_multiplier(
        &self,
        kind: UnitKind,
        action: &str,
        is_work_action: bool,
    ) -> f32 {
        let Some(overrides) = self.unit_overrides(kind) else {
            return if is_work_action {
                self.defaults.work_cycle_duration_multiplier
            } else {
                1.0
            };
        };
        if let Some((_, pacing)) = overrides
            .actions
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(action))
        {
            return pacing.cycle_duration_multiplier;
        }
        if is_work_action {
            return overrides
                .work_cycle_duration_multiplier
                .unwrap_or(self.defaults.work_cycle_duration_multiplier);
        }
        1.0
    }

    fn unit_overrides(&self, kind: UnitKind) -> Option<&UnitPacingOverrides> {
        let key = kind
            .conquests_id()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{kind:?}").to_ascii_lowercase());
        self.units.get(&key)
    }

    fn validate(&self) -> Result<(), String> {
        let mut values = vec![
            (
                "defaults.work_cycle_duration_multiplier",
                self.defaults.work_cycle_duration_multiplier,
            ),
            (
                "defaults.fidget_play_seconds",
                self.defaults.fidget_play_seconds,
            ),
            (
                "defaults.fidget_rest_seconds",
                self.defaults.fidget_rest_seconds,
            ),
        ];
        for (unit, overrides) in &self.units {
            if let Some(value) = overrides.work_cycle_duration_multiplier {
                values.push(("unit work cycle multiplier", value));
            }
            if let Some(fidget) = overrides.fidget {
                values.push(("unit fidget play_seconds", fidget.play_seconds));
                values.push(("unit fidget rest_seconds", fidget.rest_seconds));
            }
            for pacing in overrides.actions.values() {
                values.push((
                    "unit action cycle multiplier",
                    pacing.cycle_duration_multiplier,
                ));
            }
            for action in overrides.actions.keys() {
                if !is_known_action(action) {
                    return Err(format!(
                        "unit animation pacing for '{unit}' names unknown action '{action}'"
                    ));
                }
            }
            if !UnitKind::ALL_NATIVE.iter().any(|kind| {
                kind.conquests_id()
                    .is_some_and(|id| id.eq_ignore_ascii_case(unit))
            }) {
                return Err(format!("unit animation pacing names unknown unit '{unit}'"));
            }
        }
        if let Some((name, _)) = values
            .into_iter()
            .find(|(_, value)| !value.is_finite() || *value <= 0.0)
        {
            return Err(format!(
                "unit animation pacing value '{name}' must be finite and greater than zero"
            ));
        }
        Ok(())
    }
}

fn is_known_action(action: &str) -> bool {
    [
        "default",
        "blank",
        "walk",
        "run",
        "attack",
        "attack2",
        "attack3",
        "defend",
        "death",
        "dead",
        "victory",
        "fidget",
        "fortify",
        "fortifyhold",
        "turnleft",
        "turnright",
        "stop_at_last_frame",
        "build",
        "road",
        "pauseroad",
        "mine",
        "pausemine",
        "irrigate",
        "pauseirrigate",
        "forest",
        "pauseforest",
        "jungle",
        "plant",
        "fortress",
        "capture",
    ]
    .iter()
    .any(|known| known.eq_ignore_ascii_case(action))
}
