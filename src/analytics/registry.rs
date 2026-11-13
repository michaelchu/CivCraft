//! Explicit registration keeps plugins inspectable and deterministic.
use super::{
    Analyzer, Registry,
    analyzers::{growth::Growth, production::Production, settlement::Settlement},
};

pub(crate) const IDS: &[&str] = &["growth", "settlement", "production"];
pub(crate) fn build(
    names: &[String],
    growth_stall_turns: u32,
    settlement_every: i32,
) -> Result<Registry, String> {
    let mut analyzers: Vec<Box<dyn Analyzer>> = Vec::new();
    for name in names {
        analyzers.push(match name.as_str() {
            "growth" => Box::new(Growth::new(growth_stall_turns)),
            "settlement" => Box::new(Settlement::new(settlement_every)),
            "production" => Box::new(Production::default()),
            _ => {
                return Err(format!(
                    "unknown analyzer {name}; available: {}",
                    IDS.join(",")
                ));
            }
        });
    }
    Registry::new(analyzers)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn registration_is_validated_and_requirements_are_declared() {
        assert!(build(&["missing".into()], 2, 10).is_err());
        assert!(build(&["growth".into(), "growth".into()], 2, 10).is_err());
        let registry = build(&["production".into()], 2, 10).unwrap();
        assert!(registry.providers().is_empty());
        assert!(registry.event_kinds().contains("production.completed"));
    }
}
