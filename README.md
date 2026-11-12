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
