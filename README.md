This build contains the original game's own files, for the private use of the person who owns the game, on their own computer. It must not be shared or published.

# CivCraft

CivCraft is a focused, offline Rust recreation of Freeciv's opening-era single-player game. It starts with the Civ2Civ3 setup, lets six civilizations explore a wrapped isometric-hex world, found and grow cities, research the Civ2Civ3 technology tree, improve land, produce units, fight with terrain-aware combat, capture rival cities, and win by conquest.

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

## Research

Lines connect each advance to its prerequisites, and gold highlights the branch leading to your goal. Right or middle drag to pan, scroll to zoom, and use **Fit tree** for an overview or **Find goal** to return to your goal. Hover an advance to see its prerequisites and unlocks. Selecting a new goal only discards accumulated research if it changes the current target. Philosophy gives a free advance to the first civilization to discover it. After the named advances are complete, research continues with repeatable Future Tech.

The research screen distinguishes units that CivCraft can produce from unit and building unlocks in the reference. The Wheel unlocks Chariots and Mathematics unlocks Catapults. Later advances can be researched while their associated game systems are still under development.

## Automated modes

--screenshot followed by a PNG path renders 120 frames, saves the current game view, and exits.

--self-test followed by a JSON path plays a scripted opening, checks movement, city founding, and the HUD from game state, writes the result, and exits.

## Repository

The project is hosted at [michaelchu/CivCraft](https://github.com/michaelchu/CivCraft). Keep the repository private because it includes original Freeciv artwork.

## Focus of this build

This first version is the complete opening-era offline skirmish loop. It does not yet reproduce network multiplayer, every government and historical era, the full diplomacy and espionage dialogs, naval and air warfare, wonders and spaceship victory, save files, scenarios, Lua rulesets, or the map editor. The source tree contains several different desktop clients, so this build uses the normal Amplio2 isometric art with a compact SDL-style game layout rather than copying one operating-system window exactly.
