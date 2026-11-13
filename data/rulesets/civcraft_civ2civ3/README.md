# CivCraft `civ2civ3` Ruleset Baseline

This directory is CivCraft's editable baseline derived from Freeciv's
`civ2civ3` ruleset. Its sole Freeciv source is revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`.

## Contents and source

- The eleven top-level `*.ruleset` files come from Freeciv's
  `data/civ2civ3/` directory.
- `README.civ2civ3` is retained because `game.ruleset` references it for the
  Freeciv ruleset editor.
- `script.lua` and `../default/default.lua` are retained as source references.
  CivCraft does not execute the Lua files; their gameplay hooks still need
  Rust implementations under the later campaign-system milestones.
- The sibling `default/`, `nation/`, and `override/` files are the recursive
  include closure required by `nations.ruleset` and
  `default/nationlist.ruleset`. They are shared C2C3 data dependencies, not
  another gameplay ruleset.
- `COPYING` preserves the upstream Freeciv license text. The upstream ruleset
  text does not carry a separate per-file licensing statement in this source
  checkout; do not infer one from this manifest.
- `UPSTREAM_MANIFEST.tsv` records source and copied SHA-256 hashes for every
  upstream file. `identical` means the file is unchanged; `modified` means
  CivCraft changed the copy and the source hash still identifies the upstream
  original.

The `game.ruleset` copy has intentional changes: its ruledit description
paths point to `civcraft_civ2civ3/README.civ2civ3`, and its `[settings]` table
records `victories = "SPACERACE|ALLIED"`, `landmass = 30`,
`startpos = "DEFAULT"`, `specials = 250`, and `huts = 15` explicitly. These
settings are inherited engine defaults that the upstream C2C3 table does not
record: `victories` comes from `common/game.h::GAME_DEFAULT_VICTORY_CONDITIONS`,
`landmass` and `startpos` from `common/map.h::MAP_DEFAULT_LANDMASS` and
`MAP_DEFAULT_STARTPOS`, and `specials` and `huts` from `MAP_DEFAULT_RICHES` and
`MAP_DEFAULT_HUTS`. The ruleset model validates the pinned ranges and typed
values. Recording these inputs makes terrain, resource, and hut generation
editable without relying on implicit server defaults. The `ocean_resources`
parameter remains selected from C2C3 `terrain.ruleset` (default `FALSE`), where
it controls whether resource extras may appear away from safe coastline. No
files or values are sourced from `classic` or another Freeciv gameplay ruleset.
All eleven required gameplay files are present in this selected directory; the
sibling `default/` folder contains include and script data only.

For P8, the same `[settings]` table also records seven pinned Freeciv engine
defaults that C2C3 does not set itself: `sciencebox = 100`, `techpenalty = 100`,
`techleak = 100`, `techlost_recv = 0`, `techlost_donor = 0`,
`diplbulbcost = 0`, and `revolen = 5`. It also records
`happyborders = "NATIONAL"` so unit unrest follows an editable, explicit
setting rather than a hidden engine default.
Their values come from `common/game.h` (`GAME_DEFAULT_SCIENCEBOX`,
`GAME_DEFAULT_TECHPENALTY`, `GAME_DEFAULT_TECHLEAK`,
`GAME_DEFAULT_TECHLOST_RECV`, `GAME_DEFAULT_TECHLOST_DONOR`, and
`GAME_DEFAULT_DIPLBULBCOST`, `GAME_DEFAULT_REVOLUTION_LENGTH`, and
`GAME_DEFAULT_HAPPYBORDERS`) in revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`. C2C3's explicit
`techlossforgiveness = 0`, `techlossrestore = 50`, and
`revolentype = "RANDQUICK"` remain unchanged. This keeps the effective P8
inputs editable in the selected derivative instead of depending on implicit
server defaults.

P8's `unit_economics.tsv` maps all 92 Conquests unit IDs to selected C2C3 unit
records for upkeep and supported-unit happiness only: 33 direct adapters and
59 explicit role aliases. The aliases for Army, Curragh, Flak, Mobile SAM,
Privateer, and TOW Infantry are economics profiles, not claims that Freeciv
and Civ III unit identities match. Civ III identity, combat values, movement,
and presentation remain on the Civ III catalog; the adapter consumes C2C3
`uk_*` values and effects. Each bridge rationale is recorded in the table.

The included Freeciv nation files preserve the native ruleset's include graph.
CivCraft's playable civilization roster remains the Civ III catalog in
`../civ3_conquests/civilizations.tsv`; the future content adapter must keep
that roster authoritative when the ruleset data is connected to gameplay.

## License and distribution note

Freeciv code and script files carry GPL terms in the pinned checkout, and its
root `COPYING` is included here. This records provenance; it does not determine
whether a combined CivCraft distribution may include proprietary Civ III art
or satisfy all licensing requirements. Review redistribution terms before
publishing a combined build.
