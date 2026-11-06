# CivCraft

CivCraft is an offline Rust recreation of Freeciv's single-player game using the supplied Civ2Civ3 rules as its reference. Six civilizations explore a horizontally wrapped isometric-hex world, develop cities, research technologies, negotiate treaties, trade, wage land, sea, and air warfare, and pursue conquest or spaceship victory.

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
- Click a city to open city management, change production, or select a garrisoned unit. Click the research button to browse all 87 Civ2Civ3 advances and select a research goal. CivCraft automatically researches the missing prerequisites in order.
- N starts a fresh world after victory or defeat; Escape cancels a queued goto.

The right side of the screen holds the world overview, objective, research, selected-unit or city details, actions, and event history. The bottom strip always shows the selected unit's health, movement, attack, defense, and terrain.

## Map seeds

CivCraft generates a new map locally each time it launches and shows the world seed above the minimap and prints it to the terminal. To replay a world, pass that seed back in:

```sh
cargo run --release -- --seed 12345
```

## Save and resume

Press F5 to quicksave and F9 to load the quicksave. CivCraft also writes a separate autosave after each completed turn. The files are stored in the working directory.

To resume the autosave from the command line:

```sh
cargo run --release -- --load civcraft-autosave.save
```

## Research

Lines connect each advance to its prerequisites, and gold highlights the branch leading to your goal. Right or middle drag to pan, scroll to zoom, and use **Fit tree** for an overview or **Find goal** to return to your goal. Hover an advance to see its prerequisites and unlocks. Selecting a new goal only discards accumulated research if it changes the current target. Philosophy gives a free advance to the first civilization to discover it. After the named advances are complete, research continues with repeatable Future Tech.

The research screen lists unit and building unlocks from the implemented catalog, including all 57 Civ2Civ3 unit types. Production still requires the relevant technology, government, terrain access and other prerequisites; special scenario units remain unbuildable. Some associated systems and special-unit behavior are still under development.

## Automated modes

--screenshot followed by a PNG path renders 120 frames, saves the current game view, and exits.

--self-test followed by a JSON path runs the desktop gameplay scenarios, including city management, diplomacy, warfare, AI advisors, and save migration, writes the results, and exits. Rust regression tests run with `cargo test --offline`. Failed desktop checks return a nonzero exit status.

`--ui-review /tmp/civcraft-ui --seed 42` captures the visual gallery and writes a JSON report. Add `--ui-scene touching-borders` (or another scene name) for a focused run. The gallery includes normal campaign flows driven through UI hit testing; see [UI_REVIEW.md](UI_REVIEW.md) for scenarios, profiling details, and validation limits.

## Repository

The project is hosted at [michaelchu/CivCraft](https://github.com/michaelchu/CivCraft).

## Focus of this build

The implementation focuses on local single-player gameplay with the bundled Civ2Civ3 rules. It includes governments, diplomacy and espionage, naval and air warfare, wonders, and spaceship progression. Exact Freeciv equivalence is still under development; see [MECHANICS.md](MECHANICS.md) for reference comparisons and remaining AI differences. Network multiplayer, arbitrary Lua rulesets, scenarios, and the map editor remain outside this build's scope. The source tree contains several different desktop clients, so this build uses the normal Amplio2 isometric art with a compact SDL-style game layout rather than copying one operating-system window exactly.
