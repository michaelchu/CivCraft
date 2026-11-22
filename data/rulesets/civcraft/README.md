# CivCraft campaign ruleset

This directory is the CivCraft campaign content package. Its editable layer is
`content/`, containing CivCraft-owned native TOML catalogs. Internal engine
mechanics (actions, effects, extras, settings) are Rust tables in
`src/rules/engine_rules/`.

The catalogs under `content/` are the runtime source for the migrated campaign systems.  The
legacy extraction tables and one-time conversion tool have been removed after
the generated TOML catalogs were verified against them.  Edit the TOML files
and keep the focused catalog and parity tests authoritative; campaign content is now independent of BIQ records.
See [the configuration contract](../../../docs/RULESET_CONFIGURATION.md).

Current catalogs:

- `content/units.toml`: 92 native unit identities and tuneable values (defaults omitted).
- `content/experience_levels.toml`: Conquests experience ranks, hit points, and promotion values.
- `content/civilizations.toml`: 31 civilizations, city lists, starting technologies,
  and starting-unit profiles.
- `content/buildings.toml`: 91 active rules with stable IDs, named requirements,
  and `selected_obsolescence` policy flags.
- `content/technologies.toml`: 85 native technology identities, costs,
  prerequisites, eras, and research metadata.
- `content/religion.toml`: custom religion founding, spread, faith bonuses, and policy rules.

Values that are not tuning knobs (unit art folders, technology save indices,
civilization palette/city-style bindings, bombardment effect flags) live in
`src/rules/native/bindings.rs`, not in these files. Default-valued fields are
omitted, so a record lists only what differs from the default.

Economy/resource overrides, settlement/Great Leader/village tables, AI strategy,
diplomacy/vassalage policies, and the eight campaign governments live directly in
Rust (`src/rules/native/` and `src/rules/policies.rs`).

The campaign embeds its native catalogs when built. The
effect graph, including government rate caps, empire size and civil-war chance, is
`src/rules/engine_rules/effect_table.rs`, and action enablers are
`src/rules/engine_rules/action_enablers.rs`. Terrain yields, movement, activity timing, and climate
live in `src/rules/terrain_catalog.rs`, while `src/rules/engine_rules/extras.rs` holds
extras and roads.

Unit shield costs are authoritative in `content/units.toml` (`cost_shields`). Production,
settlement AI valuation, and shield-recovery base values share that cost.
The unit-economics crosswalk has been removed. Capabilities and AI roles use named fields rather than raw PRTO masks; experience and government support use their native catalogs.
Barbarian and partisan choices are configured in `src/rules/native/` alongside
the settlement rules.
The generic engine's remaining algorithm notes live in docs; removal of inherited
content does not claim full gameplay parity.

Animation pacing lives in `data/presentation/unit_animation_pacing.toml`; it does not define simulation behavior. BIQ files and extracted metadata are no longer required.
