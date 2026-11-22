# Engine rules provenance

CivCraft's engine rules are Rust, in `src/rules/engine_rules/`. They were
derived from Freeciv's `civ2civ3` ruleset, revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`, and this directory records where
they came from. Nothing here is read at build or run time.

Player-tuneable content lives separately in
[`data/rulesets/civcraft/content/`](../../../../data/rulesets/civcraft/content/).

## Where each part of the ruleset lives

| Freeciv file | Now |
| --- | --- |
| `effects.ruleset` | `effect_table.rs`, the effect graph |
| `actions.ruleset` | `action_enablers.rs`, the action enablers |
| `terrain.ruleset` | `extras.rs` for extras and roads; `src/sim/movement_rules.rs` for `move_fragments` and `igter_cost` |
| `units.ruleset` | `identifiers.rs` for unit and class flag names; `src/sim/movement_rules.rs` for unit class movement data |
| `game.ruleset` | `campaign.rs` for settings; `disasters.rs`; `src/sim/achievements.rs` |

Action parameters that were in `actions.ruleset` are constants beside the code
that uses them: the bombard block list in `src/sim/aviation.rs`, the airlift
settings in `src/sim/aviation/paradrop.rs`, and the nuclear weapon-consumption
rule in `src/sim/nuclear.rs`. Economy, settlement, strategy, diplomacy,
vassalage and government data are in `src/rules/native.rs` and
`src/rules/policies.rs`; terrain yields and climate are in
`src/rules/terrain_catalog.rs`.

The tables keep Freeciv's rule names so they stay comparable with the upstream
source, but they have diverged where CivCraft's rules do.

## Files

- `COPYING` preserves the upstream Freeciv license text. The upstream ruleset
  text does not carry a separate per-file licensing statement in this source
  checkout; do not infer one from this manifest.
- `UPSTREAM_MANIFEST.tsv` records the original upstream import, including rows
  for files and nation records CivCraft later removed. `identical` and
  `modified` describe the imported copy at the time recorded, not necessarily
  the current file. Its destination paths use the layout the files had when
  they were imported (`data/rulesets/civcraft/engine/`), and those files no
  longer exist.
- No file or value is sourced from `classic` or another Freeciv gameplay
  ruleset.

## License and distribution note

Freeciv code and script files carry GPL terms in the pinned checkout, and its
root `COPYING` is included here. This records provenance; it does not determine
whether a combined CivCraft distribution may include proprietary Civ III art
or satisfy all licensing requirements. Review redistribution terms before
publishing a combined build.
