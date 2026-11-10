# CivCraft

The [Civ III rules catalog](CIV3_MANUAL_RULES.md) records sourced terrain and selected gameplay rules. The [gameplay gap audit](GAMEPLAY_GAP_AUDIT.md) is a worksheet for comparing those rules with the implementation; that broader audit has not started.

CivCraft is an offline Rust strategy game whose intended gameplay target is Civilization III. Its current implementation includes legacy Freeciv/Civ2Civ3-derived systems that have not yet been fully audited against the supplied Civ III manual. Six civilizations explore a horizontally wrapped, staggered-square world, develop cities, research technologies, negotiate treaties, trade, fight land, sea, and air wars, and pursue conquest or spaceship victory.

## Civilizations

The fixed roster uses six civilizations from Civilization III. You lead the Romans; the other five are AI opponents.

| Civilization | Leader | First capital |
| --- | --- | --- |
| Romans | Julius Caesar | Rome |
| Greeks | Alexander | Athens |
| Egyptians | Cleopatra | Thebes |
| Babylonians | Hammurabi | Babylon |
| Chinese | Mao Zedong | Beijing |
| Japanese | Tokugawa | Kyoto |

Each civilization has its own ordered city names and Civ III city style. City names skip names already used anywhere in the world, then use “New” names when the list runs out. Captured cities keep their names. Save loading accepts only the current format; older saves are unsupported. The campaign catalog includes the standard Conquests unit roster, including unique units, but Civ III civilization traits and some unit-specific rules are not fully represented.

## Play

Run cargo run --release from this folder. The match opens directly on the map.

- Left click selects your units and cities.
- Arrow keys or 2, 3, 4, 6, 7, 8 move the selected unit in the world's six directions.
- G then left click sends a selected unit toward a tile.
- B founds a city with Settlers.
- R builds a road, I irrigates, and M builds a mine with Workers or Settlers.
- F fortifies a military unit; X automates an Explorer; A automates a Worker.
- Space finishes the selected unit and selects the next one.
- Shift plus Enter ends the turn. Shift plus arrow keys pan the map.
- The mouse wheel zooms; middle-drag or right-drag pans.
- Click a city to open city management, change production, or select a garrisoned unit. Click the research button to browse the current 87-advance catalog and select a research goal. CivCraft automatically researches the missing prerequisites in order.
- N starts a fresh world after victory or defeat; Escape cancels a queued goto.

The right side of the screen holds the world overview, objective, research, selected-unit or city details, actions, and event history. The bottom strip always shows the selected unit's health, movement, attack, defense, and terrain.

## Map seeds

CivCraft generates a new map locally each time it launches and shows the world seed above the minimap and prints it to the terminal. To replay a world, pass that seed back in:

```sh
cargo run --release -- --seed 12345
```

New maps default to Civ III's 70% ocean coverage, Normal climate, Continents
landform, Temperate temperature, and 4 billion year age. Select ocean
coverage with `--ocean-coverage 60`, `70`, or `80`, climate with `--climate
arid`, `normal`, or `wet`, landform with `--landform archipelago`,
`continents`, or `pangaea`, and temperature with `--temperature cool`,
`temperate`, or `warm`. World age uses `--world-age 3`, `4`, or `5` billion
years:

```sh
cargo run --release -- --seed 42 --map-size 100x100 --ocean-coverage 60 --climate wet --world-age 3 --landform archipelago --temperature warm
```

Choose Civ III's Tiny, Small, Standard, Large, or Huge world with
`--map-size tiny|small|standard|large|huge`; Standard (100×100) is the
default. Mods and custom scenarios can use `--map-size WIDTHxHEIGHT` with even
dimensions from 16 to 512 tiles per side.

## Map wrapping

New games use Civ III's horizontal wrap with bounded polar edges:

```sh
cargo run --release -- --seed 42
```

The playable map does not generate land on its top and bottom edge rows. The
Civ III `polarICEcaps-final.png` artwork supplies the visual polar caps outside
the playable map. Campaign maps wrap horizontally and have bounded polar
edges. The former WrapXY mode and older save formats are unsupported.

The renderer and primary generation, movement, pathfinding, city work-area,
and visibility paths use Civ III's staggered grid. The options above are
selectable, while landform and temperature use independently seeded
subdivision fields with OpenCiv3-derived percentile bands. Their noise
algorithm and spatial scale remain approximations of OpenCiv3's OpenSimplex
maps.

## Map tilesets

The game reads original terrain art from a sibling `source/` folder at startup.
Place `source/` beside `app/`, containing the original `Conquests/Art`,
`civ3PTW/Art`, and base `Art` folders. Conquests takes priority, with PTW and
base Civ III filling missing paths. CivCraft reports an error and exits if the
source art folder is unavailable. Snoopy terrain variants are read directly
from their PTW scenario folders. Select a variant with:

```sh
cargo run --release -- --tileset snoopy-standard --seed 42
cargo run --release -- --tileset snoopy-greener --seed 42
cargo run --release -- --tileset snoopy-extended --seed 42
```

The selection applies to terrain on the world map and city work map. Other art
continues to use its existing paths while it is migrated separately. Unit art
can be overridden per unit
with an FLC animation from a mod; units without configured FLC art use a vector
map marker. Compact non-map panels retain a small shared unit icon atlas.
Resource icons come from the selected theme’s resource sheet. A missing sheet
or a transparent icon cell uses the existing fallback resource icon.

The Civ III geometry backend is the only supported renderer. It controls map
projection and tile picking.

Use `--tileset-theme PATH` with `--tileset civ3` to select a local art directory
or a manifest file. An art directory can follow the original filenames, with
`Terrain/` and the resource sheets at its root. A directory may also contain a
`tileset.conf` file. Entries are `role = path`, with relative paths resolved from
the manifest's directory. Sheets retain the Civ III cell layout.

| Roles | Default filenames under the art directory |
| --- | --- |
| `desert-grass-plains`, `desert-grass-coast`, `desert-plains-coast`, `plains-grass-coast`, `tundra-grass-coast`, `grass-coast` | `Terrain/XDGP.png`, `XDGC.png`, `XDPC.png`, `XPGC.png`, `XTGC.png`, `XGGC.png` |
| `water`, `ocean`, `polar` | `Terrain/wCSO.png`, `wOOO.png`, `polarICEcaps-final.png` |
| `vegetation`, `marsh`, `hills`, `mountains` | `Terrain/grassland forests (normal).png`, `marsh.png`, `XHILLS.png`, `Mountains.png` |
| `irrigation`, `irrigation-plains`, `irrigation-desert`, `irrigation-tundra` | `Terrain/irrigation.png`, `irrigation PLAINS.png`, `irrigation DESETT.png`, `irrigation TUNDRA.png` |
| `roads`, `railroads` | `Terrain/roads.png`, `Terrain/railroads.png` |
| `terrain-buildings`, `pollution`, `craters` | `Terrain/TerrainBuildings.png`, `Terrain/pollution.png`, `Terrain/craters.png` |
| `oil-well`, `oil-platform`, `airstrip`, `airbase` | Optional one-cell `Terrain/<role>.png` sprites |
| `resources`, `resource-shadows` | `resources.png`, `resources_shadows.png` |
| `city-asian`, `city-american`, `city-european`, `city-middle_eastern`, `city-roman` | `Cities/rASIAN.png`, `rAMER.png`, `rEURO.png`, `rMIDEAST.png`, `rROMAN.png` |
| `unit-<kind>`, `unit-<kind>-run`, `unit-<kind>-attack` | Optional `.flc` default, movement, and attack animations, or a unit `.ini` that maps them; for example `unit-warrior = Units/Warrior/Warrior.ini` |

Each image role accepts PNG or 8-bit, single-plane RLE PCX artwork. If the default
PNG path is absent, the loader checks the same basename with `.pcx` and `.PCX`.
PCX magenta color-key pixels are decoded as transparent. City-style roles can
be overridden in `tileset.conf` like the terrain and resource roles.

Unit roles use lowercase kebab-case `UnitKind` names (`unit-stealth-fighter`,
`unit-mech-inf`, `unit-settlers`). Point a base role to an FLC or Civ III unit
`.ini`; the loader reads its `DEFAULT`, `RUN`, and `ATTACK1` entries and resolves
those FLC paths relative to the `.ini`. Optional explicit `-run` and `-attack`
roles override the corresponding `.ini` entries. Missing action roles fall back
to the default FLC. Frames are packed into direction/frame atlases and selected
using event progress, with palette team-color and shadow ranges preserved.
Movement currently supports six facing directions due to the legacy simulation
topology; full eight-way movement, broader Civ III action states, and automatic
mapping of the full unit roster remain unfinished.

The ground, water, and polar sheets are required. The other groups are optional.
For two interactive windows, launch the binary from separate working
directories so each instance has its own saves. For screenshot comparisons,
use the same seed, wrap mode, window size, and scene with different
`--ui-review` output directories.

## Save and resume

Press F5 to quicksave and F9 to load the quicksave. CivCraft also writes a separate autosave after each completed turn. The files are stored in the working directory.

To resume the autosave from the command line:

```sh
cargo run --release -- --load civcraft-autosave.save
```

## Research

Lines connect each advance to its prerequisites, and gold highlights the branch leading to your goal. Right or middle drag to pan, scroll to zoom, and use **Fit tree** for an overview or **Find goal** to return to your goal. Hover an advance to see its prerequisites and unlocks. Selecting a new goal only discards accumulated research if it changes the current target. Philosophy gives a free advance to the first civilization to discover it. After the named advances are complete, research continues with repeatable Future Tech.

The research screen lists unit and building unlocks from the implemented catalog, including 92 standard Conquests campaign unit identities. Production checks the requirements represented by the implementation; whether these checks match the manual has not yet been broadly audited. Some associated systems and special-unit behavior are still under development.

## Automated modes

--screenshot followed by a PNG path renders 120 frames, saves the current game view, and exits.

--self-test followed by a JSON path runs the desktop gameplay scenarios, including city management, diplomacy, warfare, and AI advisors, writes the results, and exits. Rust regression tests run with `cargo test --offline`. Failed desktop checks return a nonzero exit status.

`--ui-review /tmp/civcraft-ui --seed 42` captures the visual gallery and writes a JSON report. Add `--ui-scene touching-borders` (or another scene name) for a focused run. The gallery includes normal campaign flows driven through UI hit testing; see [UI_REVIEW.md](UI_REVIEW.md) for scenarios, profiling details, and validation limits.

## Repository

The project is hosted at [michaelchu/CivCraft](https://github.com/michaelchu/CivCraft).

## Focus of this build

The implementation includes governments, diplomacy and espionage, naval and air warfare, wonders, and spaceship progression. The rules catalog contains sourced expectations, while the broader manual-to-code gameplay audit remains outstanding. Network multiplayer, arbitrary Lua rulesets, scenarios, and the map editor remain outside this build's scope. This build uses Civ III map artwork with a compact SDL-style game layout.

Current map-rendering behavior and visual-review guidance are documented in
[UI_REVIEW.md](UI_REVIEW.md).
