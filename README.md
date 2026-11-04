This build contains the original game's own files, for the private use of the person who owns the game, on their own computer. It must not be shared or published.

# CivCraft

CivCraft is a focused, offline Rust recreation of Freeciv's opening-era single-player game. It starts with the Civ2Civ3 setup, lets six civilizations explore a wrapped isometric-hex world, found and grow cities, research early advances, improve land, produce units, fight with terrain-aware combat, capture rival cities, and win by conquest.

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
- Click a city to open city management, change production, or select a garrisoned unit. Click the research card to change the current advance.
- N starts a fresh world after victory or defeat; Escape cancels a queued goto.

The right side of the screen holds the world overview, objective, research, selected-unit or city details, actions, and event history. The bottom strip always shows the selected unit's health, movement, attack, defense, and terrain.

## Automated modes

--screenshot followed by a PNG path renders 120 frames, saves the current game view, and exits.

--self-test followed by a JSON path plays a scripted opening, checks movement, city founding, and the HUD from game state, writes the result, and exits.

## Repository

The project is hosted at [michaelchu/CivCraft](https://github.com/michaelchu/CivCraft). Keep the repository private because it includes original Freeciv artwork.

## Focus of this build

This first version is the complete opening-era offline skirmish loop. It does not yet reproduce network multiplayer, every government and historical era, the full diplomacy and espionage dialogs, naval and air warfare, wonders and spaceship victory, save files, scenarios, Lua rulesets, or the map editor. The source tree contains several different desktop clients, so this build uses the normal Amplio2 isometric art with a compact SDL-style game layout rather than copying one operating-system window exactly.
