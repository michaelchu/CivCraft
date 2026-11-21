//! Validated mechanical and AI thresholds from content data.
pub(super) use crate::rules::native::VassalageCatalog as Policy;

pub(super) fn policy() -> &'static Policy {
    crate::rules::native::vassalage()
}
