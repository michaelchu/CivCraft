# CivCraft

CivCraft is an offline Rust strategy game targeting Civilization III: Conquests.
Its civilization catalog supports all 31 Conquests civilizations:

| Civilization | Civilization | Civilization |
| --- | --- | --- |
| America | Arabia | Aztecs |
| Babylonians | Byzantines | Carthage |
| Celts | Chinese | Egyptians |
| England | France | Germany |
| Greeks | Hittites | Inca |
| India | Iroquois | Japanese |
| Korea | Maya | Mongols |
| Netherlands | Ottomans | Persia |
| Portugal | Romans | Russia |
| Scandinavia | Spain | Sumeria |
| Zululand | | |

## Start a game

Set `CIVCRAFT_SOURCE_DIR` to a Civilization III installation containing the
original `Conquests/Art`, `civ3PTW/Art`, and base `Art` folders, then run:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release
```

CivCraft starts a new game with a generated seed. Pass `--seed N` to replay a
world, or add supported world setup options to the same command:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release -- \
  --seed 42 --map-size standard --ocean-coverage 70 --climate normal \
  --world-age 4 --landform continents --temperature temperate --tileset civ3
```

`--map-size` defaults to `small` (80×80). It accepts `tiny`, `small`,
`standard`, `large`, `huge`, or custom `WIDTHxHEIGHT` dimensions. Ocean
coverage accepts `60`, `70`, or `80`; climate
accepts `arid`, `normal`, or `wet`; world age accepts `3`, `4`, or `5` billion
years; landform accepts `archipelago`, `continents`, or `pangaea`; and
temperature accepts `cool`, `temperate`, or `warm`. The built-in `--tileset`
choices are `civ3`, `conquests`, `snoopy-standard`, `snoopy-greener`, and
`snoopy-extended`.

## Headless AI simulation

Run an AI-only campaign without opening a game window and write a turn-by-turn
JSON snapshot report:

```sh
cargo run --offline --release --bin headless_sim -- \
  --seed 42 --map-size tiny --ocean-coverage 70 --climate normal \
  --world-age 4 --landform continents --temperature temperate \
  --turns 100 --checkpoint-every 10 \
  --report /tmp/civcraft-simulation.json
```

The report includes each civilization's city, unit, gold, and score totals, plus
each city's population, stored food, forecast surplus, growth threshold, size
limit, disorder state, specialists, and worked-tile yields at every recorded
turn. It records unit positions, movement state, work orders, and settlement
targets; flags prolonged growth and city-founding candidates; and records turn
events. Periodic report checkpoints and progress output make long runs
observable. Scenario presets for controlled food-growth cases remain follow-up
work.

World-generation controls match the interactive game: map size accepts the
Civ III presets or `WIDTHxHEIGHT`; ocean coverage accepts `60`, `70`, or `80`;
climate accepts `arid`, `normal`, or `wet`; world age accepts `3`, `4`, or `5`;
landform accepts `archipelago`, `continents`, or `pangaea`; and temperature
accepts `cool`, `temperate`, or `warm`.
