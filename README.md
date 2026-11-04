This build contains the original game's own files, for the private use of the person who owns the game, on their own computer. It must not be shared or published.

# Open Empires

Open Empires is a focused, offline Rust recreation of Freeciv's opening-era single-player game. It starts with the Civ2Civ3 setup, lets six civilizations explore a wrapped isometric-hex world, found and grow cities, research early advances, improve land, produce units, fight with terrain-aware combat, capture rival cities, and win by conquest.

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

## Save the project to GitHub

This project includes original Freeciv artwork. Keep its GitHub repository private, and do not publish or share it.

1. On GitHub, create a new empty **private** repository named `open-empires`. Do not add a README, license, or `.gitignore` there.
2. Open Terminal, copy the commands below, and replace `YOUR_USERNAME` with your GitHub username:

```sh
cd "/Users/michaelchu/Documents/Craft Anything/freeciv/app"
git init
git add .
git commit -m "Initial Open Empires build"
git branch -M main
git remote add origin git@github.com:YOUR_USERNAME/open-empires.git
git push -u origin main
```

After that first push, future updates only need `git add .`, `git commit -m "Describe the update"`, and `git push` from this folder. The included `.gitignore` keeps local builds, test captures, logs, and macOS metadata out of commits.

## Focus of this build

This first version is the complete opening-era offline skirmish loop. It does not yet reproduce network multiplayer, every government and historical era, the full diplomacy and espionage dialogs, naval and air warfare, wonders and spaceship victory, save files, scenarios, Lua rulesets, or the map editor. The source tree contains several different desktop clients, so this build uses the normal Amplio2 isometric art with a compact SDL-style game layout rather than copying one operating-system window exactly.
