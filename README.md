# CivCraft

The manual-based gameplay rules catalog and future gap-audit worksheet are [CIV3_MANUAL_RULES.md](CIV3_MANUAL_RULES.md) and [GAMEPLAY_GAP_AUDIT.md](GAMEPLAY_GAP_AUDIT.md). Both are currently unpopulated; no gameplay audit against the manual has been completed.

CivCraft is an offline Rust strategy game whose intended gameplay target is Civilization III. Its current implementation includes legacy Freeciv/Civ2Civ3-derived systems, but those systems have not yet been audited against the supplied Civ III manual. Six civilizations explore a horizontally wrapped isometric-hex world, develop cities, research technologies, negotiate treaties, trade, wage land, sea, and air warfare, and pursue conquest or spaceship victory.

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

Each civilization has its own ordered city names and an initial Amplio2 city style. City names skip names already used anywhere in the world, then use “New” names when the list runs out. Captured cities keep their names. Existing saves retain their city names; the former six faction labels upgrade when loaded. This roster and its current defaults are implementation details, not a claim of Civ III rules fidelity; Civ III traits and unique units are not fully represented.

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

## Map wrapping

New games wrap horizontally only by default. Choose either mode at launch:

```sh
cargo run --release -- --wrap x
cargo run --release -- --wrap xy
```

`x` creates a cylinder with bounded northern/southern ocean edges and no
generated glacier or tundra. The blue ocean extends visually past the boundary
to hide its staggered outline.
`xy` wraps both axes and uses the reference preset's torus climate. Native sizing
for our six-player preset produces 36×48 tiles in `x` and 32×64 in `xy`. Use the same seed **and wrap setting** to replay a
map, for example `cargo run --release -- --seed 42 --wrap xy`. The current mode
appears next to the seed above the minimap.

The setting applies to generation, movement, pathfinding, city work areas,
visibility and rendering. Saves record it; loading uses the saved mode. An
explicit conflicting `--wrap` with `--load` is rejected rather than changing
an existing world's geography.

## Map tilesets

Amplio2 is the default. To use the Civ III artwork in
[`tilesets/civ3/`](tilesets/README.md), launch with:

```sh
cargo run --release -- --tileset civ3 --seed 42 --wrap x
```

The selection applies to the world map and the city work map. It is independent
of `--renderer` and works with new maps and existing saves. The Civ III tileset
uses the shared river, unit, and city art. Resource icons come from the selected
theme’s `resources.png`. A missing sheet or a transparent icon cell uses the
existing Amplio2-mode resource icon.

Use `--tileset-theme PATH` with `--tileset civ3` to select a local art directory
or a manifest file. An art directory can follow the original filenames, with
`Terrain/` and the resource sheets at its root. A directory may also contain a
`tileset.conf` file. Entries are `role = path`, with relative paths resolved from
the manifest's directory. Sheets retain the Civ III cell layout.

| Roles | Default PNGs under the art directory |
| --- | --- |
| `desert-grass-plains`, `desert-grass-coast`, `desert-plains-coast`, `plains-grass-coast`, `tundra-grass-coast`, `grass-coast` | `Terrain/XDGP.png`, `XDGC.png`, `XDPC.png`, `XPGC.png`, `XTGC.png`, `XGGC.png` |
| `water`, `ocean`, `polar` | `Terrain/wCSO.png`, `wOOO.png`, `polarICEcaps-final.png` |
| `vegetation`, `marsh`, `hills`, `mountains` | `Terrain/grassland forests (normal).png`, `marsh.png`, `XHILLS.png`, `Mountains.png` |
| `irrigation`, `irrigation-plains`, `irrigation-desert`, `irrigation-tundra` | `Terrain/irrigation.png`, `irrigation PLAINS.png`, `irrigation DESETT.png`, `irrigation TUNDRA.png` |
| `resources`, `resource-shadows` | `resources.png`, `resources_shadows.png` |

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

The research screen lists unit and building unlocks from the implemented catalog, including 57 current unit types. Production checks the requirements represented by the implementation; whether these checks match the manual has not yet been audited. Some associated systems and special-unit behavior are still under development.

## Automated modes

--screenshot followed by a PNG path renders 120 frames, saves the current game view, and exits.

--self-test followed by a JSON path runs the desktop gameplay scenarios, including city management, diplomacy, warfare, AI advisors, and save migration, writes the results, and exits. Rust regression tests run with `cargo test --offline`. Failed desktop checks return a nonzero exit status.

`--ui-review /tmp/civcraft-ui --seed 42` captures the visual gallery and writes a JSON report. Add `--ui-scene touching-borders` (or another scene name) for a focused run. The gallery includes normal campaign flows driven through UI hit testing; see [UI_REVIEW.md](UI_REVIEW.md) for scenarios, profiling details, and validation limits.

## Repository

The project is hosted at [michaelchu/CivCraft](https://github.com/michaelchu/CivCraft).

## Focus of this build

The implementation includes governments, diplomacy and espionage, naval and air warfare, wonders, and spaceship progression. The manual-based gameplay rules catalog and gap audit have not yet been populated. Network multiplayer, arbitrary Lua rulesets, scenarios, and the map editor remain outside this build's scope. The source tree contains several different desktop clients, so this build uses the normal Amplio2 isometric art with a compact SDL-style game layout rather than copying one operating-system window exactly.

Map-rendering reference comparisons, remembered-map save compatibility, terrain
artwork and go-to previews are documented in [RENDERING_PARITY.md](RENDERING_PARITY.md).
