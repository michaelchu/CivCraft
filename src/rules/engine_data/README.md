# CivCraft engine layer

This directory holds the Freeciv-format rule data that CivCraft still parses at
startup. It is derived from Freeciv's `civ2civ3` ruleset, revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`. The files are embedded in the
binary with `include_str!` (`src/rules/engine_rules/loader.rs`); nothing is read
from disk at runtime, and they are not a player-editable layer. Player-tuneable
content lives in [`../../../data/rulesets/civcraft/content/`](../../../data/rulesets/civcraft/content/).

The layer is being moved into Rust. What remains here:

| File | Still used for |
| --- | --- |
| `effects.ruleset` | The effect graph: government rate caps, vision, empire size, civil war and similar effects |
| `terrain.ruleset` | Extras, roads, resources and movement parameters |

## What moved to code

The campaign's game, research, combat, trade, calendar and veterancy settings
are Rust constants in
[`src/rules/engine_rules/campaign.rs`](../engine_rules/campaign.rs). Each one
names the Freeciv section or server setting it comes from, and the pinned
engine default where the ruleset leaves the value unset. Economy, settlement,
strategy, diplomacy, vassalage and government data are in
`src/rules/native.rs` and `src/rules/policies.rs`. Terrain yields and climate
are in `src/rules/terrain_catalog.rs`.

The action enablers are a Rust table in
[`src/rules/engine_rules/action_enablers.rs`](../engine_rules/action_enablers.rs),
the disasters are in `src/rules/engine_rules/disasters.rs`, and the achievements
are in `src/sim/achievements.rs`. The unit class behavior is in
`src/sim/movement_rules.rs`, and the flag and class names that requirements are
checked against are in `src/rules/engine_rules/identifiers.rs`.

A few action parameters that were in `actions.ruleset` are constants next to
the code that uses them: the bombard block list in `src/sim/aviation.rs`, the
airlift settings in `src/sim/aviation/paradrop.rs`, and the nuclear
weapon-consumption rule in `src/sim/nuclear.rs`.

## Provenance

- `COPYING` preserves the upstream Freeciv license text. The upstream ruleset
  text does not carry a separate per-file licensing statement in this source
  checkout; do not infer one from this manifest.
- `UPSTREAM_MANIFEST.tsv` records the original upstream import, including rows
  for files and nation records CivCraft later removed. `identical` and
  `modified` describe the imported copy at the time recorded, not necessarily
  the current file. Its destination paths use the layout the files had when
  they were imported (`data/rulesets/civcraft/engine/`).
- No file or value is sourced from `classic` or another Freeciv gameplay
  ruleset. CivCraft's playable civilizations come from the Civ III catalog in
  `../../../data/rulesets/civcraft/content/civilizations.toml`.

## License and distribution note

Freeciv code and script files carry GPL terms in the pinned checkout, and its
root `COPYING` is included here. This records provenance; it does not determine
whether a combined CivCraft distribution may include proprietary Civ III art
or satisfy all licensing requirements. Review redistribution terms before
publishing a combined build.
