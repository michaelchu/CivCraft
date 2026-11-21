# CivCraft

CivCraft is a 4X turn-based strategy game written in Rust, combining Freeciv gameplay rules with *Civilization III: Conquests* graphics and audio.

- **31 playable civilizations** from the full Conquests roster
- **Custom world generation** using `fractal` or `opensimplex` maps with configurable size, landform, ocean coverage, climate, temperature, and world age
- **Multiple classic tilesets**: `civ3`, `conquests`, `snoopy-standard`, `snoopy-greener`, and `snoopy-extended`

## Playable Civilizations

- America
- Arabia
- Aztecs
- Babylonians
- Byzantines
- Carthage
- Celts
- Chinese
- Egyptians
- England
- France
- Germany
- Greeks
- Hittites
- Inca
- India
- Iroquois
- Japan
- Korea
- Maya
- Mongols
- Netherlands
- Ottomans
- Persia
- Portugal
- Rome
- Russia
- Scandinavia
- Spain
- Sumeria
- Zululand

---

## Quick Start

### Prerequisites

1. **Rust** (stable toolchain — `rust-toolchain.toml` configures required components automatically)
2. **A local Civilization III installation** containing the original `Conquests/Art`, `civ3PTW/Art`, and base `Art` directories

### Launch a Game

Point `CIVCRAFT_SOURCE_DIR` to your Civilization III install and run:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release --features desktop-audio
```

That's it—CivCraft will generate a fresh world and drop you straight into your first turn.

### Verify Your Setup (Optional)

Want a quick sanity check that your Civ III assets, renderer, and turn loop are working properly? Run the 3-turn desktop smoke test:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" ./scripts/smoke-test.sh
```

*(Set `CIVCRAFT_SMOKE_TURNS` to adjust the turn count. The script exits cleanly with `SKIP` if `CIVCRAFT_SOURCE_DIR` is unset or missing `Conquests/Art/Terrain`.)*

---

## Customize Your World

Replay a favorite map with `--seed`, pick a tileset, or dial in custom world-generation parameters:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release --features desktop-audio -- \
  --seed 42 \
  --map-size standard \
  --map-generator opensimplex \
  --landform continents \
  --ocean-coverage 70 \
  --climate normal \
  --temperature temperate \
  --world-age 4 \
  --tileset snoopy-greener \
  --autosave true
```

### World & Game Options

| Flag | Values | Notes |
| --- | --- | --- |
| `--seed` | `<integer>` | Replay the exact same world |
| `--map-size` | `tiny`, `small` *(default, 80×80)*, `standard`, `large`, `huge`, or `WIDTHxHEIGHT` | Custom dimensions supported |
| `--map-generator` | `fractal` *(default)*, `opensimplex` | `fractal` uses the campaign ruleset |
| `--landform` | `archipelago`, `continents`, `pangaea` | Requires `--map-generator opensimplex` |
| `--ocean-coverage` | `60`, `70`, `80` | Requires `--map-generator opensimplex` |
| `--climate` | `arid`, `normal`, `wet` | Requires `--map-generator opensimplex` |
| `--temperature` | `cool`, `temperate`, `warm` | Requires `--map-generator opensimplex` |
| `--world-age` | `3`, `4`, `5` *(billion years)* | Requires `--map-generator opensimplex` |
| `--tileset` | `civ3`, `conquests`, `snoopy-standard`, `snoopy-greener`, `snoopy-extended` | Built-in terrain styles |
| `--autosave` | `true`, `false` *(default)* | Save automatically after each turn |

---

## Contributing

If you'd like to contribute or explore CivCraft's internals, check out [docs/README.md](docs/README.md) for the project documentation (including [headless AI simulations](docs/HEADLESS_ANALYTICS.md), [test scenarios](docs/SCENARIOS.md), and [asset loading](docs/CIV3_ASSET_RETRIEVAL_RECONCILIATION.md)).

To set up the local Git hooks and run formatting, Clippy, and tests:

```sh
./scripts/install-git-hooks.sh
./scripts/check-rust.sh
```
