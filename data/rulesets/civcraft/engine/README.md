# CivCraft engine layer

This directory is the engine-compatibility layer inside the CivCraft campaign
package. It is derived from Freeciv's `civ2civ3` ruleset. Its sole Freeciv source is revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`.

## Contents and source

- The eleven `*.ruleset` files come from Freeciv's
  `data/civ2civ3/` directory.
- `README.civ2civ3` is retained because `game.ruleset` references it for the
  Freeciv ruleset editor.
- `script.lua` and `../../default/default.lua` are retained as source references.
  CivCraft does not execute the Lua files; their gameplay hooks still need
  Rust implementations under the later campaign-system milestones.
- The top-level `default/` and `override/` files are shared C2C3 include and
  script dependencies, not another gameplay ruleset. CivCraft intentionally
  omits Freeciv's general nation catalog: `nations.ruleset` embeds only the
  Barbarian, Pirate, and Animal Kingdom records needed by C2C3 mechanics,
  while playable civilizations come from the Civ III source catalog.
- `COPYING` preserves the upstream Freeciv license text. The upstream ruleset
  text does not carry a separate per-file licensing statement in this source
  checkout; do not infer one from this manifest.
- `UPSTREAM_MANIFEST.tsv` records the original upstream import. It intentionally
  retains provenance rows for the general Freeciv nation catalog that CivCraft
  later removed from its runtime data tree. `identical` and `modified` describe
  the imported copy at the time recorded, not necessarily the current file.

The `game.ruleset` copy has intentional changes: its ruledit description
paths point to `civcraft/engine/README.civ2civ3`, and its `[settings]` table
records `victories = "SPACERACE|ALLIED|CULTURE"`, `landmass = 30`,
`startpos = "DEFAULT"`, `specials = 250`, and `huts = 15` explicitly. These
settings make defaults explicit where the upstream C2C3 table omits them.
The campaign adds `CULTURE` to the victory defaults in
`common/game.h::GAME_DEFAULT_VICTORY_CONDITIONS`. For map generation,
`landmass` and `startpos` use `common/map.h::MAP_DEFAULT_LANDMASS` and
`MAP_DEFAULT_STARTPOS`. The `specials` and `huts` settings use
`MAP_DEFAULT_RICHES` and `MAP_DEFAULT_HUTS`. The ruleset model validates the pinned ranges and typed
values. Recording these inputs makes terrain, resource, and hut generation
editable without relying on implicit server defaults. The `ocean_resources`
parameter remains selected from C2C3 `terrain.ruleset` (default `FALSE`), where
it controls whether resource extras may appear away from safe coastline. No
files or values are sourced from `classic` or another Freeciv gameplay ruleset.
All eleven required gameplay files are present in this engine layer; the
top-level `default/` folder contains include and script data only.

For P8, the same `[settings]` table also records eight pinned Freeciv engine
defaults that C2C3 does not set itself: `sciencebox = 100`, `techpenalty = 100`,
`techleak = 100`, `techlost_recv = 0`, `techlost_donor = 0`,
`diplbulbcost = 0`, `revolen = 5`, and `shieldbox = 100`. It also records
`happyborders = "NATIONAL"` so unit unrest follows an editable, explicit
setting rather than a hidden engine default.
Their values come from `common/game.h` (`GAME_DEFAULT_SCIENCEBOX`,
`GAME_DEFAULT_TECHPENALTY`, `GAME_DEFAULT_TECHLEAK`,
`GAME_DEFAULT_TECHLOST_RECV`, `GAME_DEFAULT_TECHLOST_DONOR`, and
`GAME_DEFAULT_DIPLBULBCOST`, `GAME_DEFAULT_REVOLUTION_LENGTH`,
`GAME_DEFAULT_SHIELDBOX`, and
`GAME_DEFAULT_HAPPYBORDERS`) in revision
`9ff184b6eb128ca636c7ad6269e27aa959600006`. C2C3's explicit
`techlossforgiveness = 0`, `techlossrestore = 50`, and
`revolentype = "RANDQUICK"` remain unchanged. This keeps the effective P8
inputs editable in the selected derivative instead of depending on implicit
server defaults.

The former P8 unit-economics crosswalk has been removed. Unit capabilities,
statistics, and support eligibility now come from native Conquests rules;
unit support allowances come from the native government catalog.

For P9, the selected C2C3 `[combat_rules]` values are loaded as required typed
inputs. Its explicit `occupychance = 100` is retained. `killstack = TRUE` and
`killcitizen = TRUE` are recorded in the derivative using the pinned engine
defaults `common/game.h::GAME_DEFAULT_KILLSTACK` and
`common/game.h::GAME_DEFAULT_KILLCITIZEN`; no combat value is borrowed from
another Freeciv ruleset. `unreachableprotects = ENABLED` records
`common/game.h::GAME_DEFAULT_UNRPROTECTS` so a unit that cannot reach one
member of a stack prevents an attack unless that unit has `NeverProtects`.
`razechance = 10` is the C2C3 value;
`savepalace = TRUE` and `homecaughtunits = TRUE` are explicit pinned engine
defaults. The `units.ruleset` derivative also records
`non_native_def_pct = 100` for each unit class, preserving the pinned
`server/ruleset/ruleload.c` engine default as editable selected-ruleset data.
Airlift source and destination capacity overrides remain explicit in
`game.ruleset`: `airlift_from_always_enabled = FALSE` and
`airlift_to_always_enabled = TRUE`; the action adapter reads both settings
alongside the selected `Airlift` effect and action range.

CivCraft's playable civilization roster is the Civ III catalog in
`../content/civilizations.toml`. The selected ruleset keeps only three
embedded special non-player nation records for barbarian and animal mechanics.

Settlement AI tuning is defined only by the `[ai]` table in
[`../content/settlements.toml`](../content/settlements.toml). It combines
Freeciv-derived advisor weights with explicit CivCraft expansion-balance
settings. Edit that TOML to change settlement AI tuning. The algorithm
reference is the `reference/freeciv`
checkout at `3efab1385e792dd367e955aee538bafd5ccadd05`; the selected C2C3 ruleset
data remains the separately pinned revision recorded above.

## License and distribution note

Freeciv code and script files carry GPL terms in the pinned checkout, and its
root `COPYING` is included here. This records provenance; it does not determine
whether a combined CivCraft distribution may include proprietary Civ III art
or satisfy all licensing requirements. Review redistribution terms before
publishing a combined build.
