# CivCraft

CivCraft is an offline strategy game written in Rust. Its gameplay rules are
based on Freeciv, with Civilization III: Conquests graphics. Choose from the
full roster of 31 civilizations:

America · Arabia · Aztecs · Babylonians · Byzantines · Carthage · Celts ·
Chinese · Egyptians · England · France · Germany · Greeks · Hittites · Inca ·
India · Iroquois · Japan · Korea · Maya · Mongols · Netherlands · Ottomans ·
Persia · Portugal · Rome · Russia · Scandinavia · Spain · Sumeria · Zululand

## Start a game

Set `CIVCRAFT_SOURCE_DIR` to a Civilization III installation containing the
original `Conquests/Art`, `civ3PTW/Art`, and base `Art` folders, then run:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release --features desktop-audio
```

CivCraft starts a new game with a generated seed. Pass `--seed N` to replay a
world, or add supported world setup options to the same command:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" cargo run --release --features desktop-audio -- \
  --seed 42 --map-size standard --map-generator opensimplex --ocean-coverage 70 --climate normal \
  --world-age 4 --landform continents --temperature temperate --tileset civ3
```

Run a short desktop smoke test to verify that the game starts, loads the Civ III
artwork, renders a frame, and advances a few normal turns:

```sh
CIVCRAFT_SOURCE_DIR="/path/to/civ3-install" ./scripts/smoke-test.sh
```

The script requires an explicit `CIVCRAFT_SOURCE_DIR` and does not guess an
installation location. It exits successfully with `SKIP` when the variable is
unset or its path does not contain `Conquests/Art/Terrain`. Set
`CIVCRAFT_SMOKE_TURNS` to change the default three turns.

`--map-size` defaults to `small` (80×80). It accepts `tiny`, `small`,
`standard`, `large`, `huge`, or custom `WIDTHxHEIGHT` dimensions. The default
`fractal` generator uses the campaign ruleset. To set ocean coverage
(`60`, `70`, `80`), climate (`arid`, `normal`, `wet`), world age
(`3`, `4`, `5` billion years), landform (`archipelago`, `continents`,
`pangaea`), or temperature (`cool`, `temperate`, `warm`), select
`--map-generator opensimplex`. The built-in `--tileset`
choices are `civ3`, `conquests`, `snoopy-standard`, `snoopy-greener`, and
`snoopy-extended`.

Media lookup supports ordered scenario/package overrides and packaged-only
operation through `CIVCRAFT_MEDIA_CONFIG`. `CIV3_HOME` is an alternative install
variable, and `CIVCRAFT_MEDIA_TRACE=1` logs resolved file provenance. See
[the media resolver and audio loader guide](docs/CIV3_ASSET_RETRIEVAL_RECONCILIATION.md)
for configuration, precedence, audio catalog keys, and standalone requirements.

## Freeciv reference source

The Git submodule at `reference/freeciv` tracks the public upstream
[Freeciv repository](https://github.com/freeciv/freeciv), using its `main` branch.
Initialize it after cloning CivCraft:

```sh
git submodule update --init reference/freeciv
```

To advance it to the latest upstream `main` commit:

```sh
git submodule update --remote reference/freeciv
git add reference/freeciv
git commit -m "Update Freeciv reference submodule"
```

CivCraft records a specific Freeciv commit for reproducible checkouts; updates
are explicit. This reference source is optional for building and running
CivCraft and is implementation context, not an authority for Civ III rules.

## Headless AI simulation

Run an AI-only campaign without opening a game window and write a
compact JSON review report with a JSONL evidence sidecar:

```sh
cargo run --offline --release --bin headless_sim -- \
  --seed 42 --map-size tiny \
  --turns 30 --checkpoint-every 10 \
  --report /tmp/civcraft-simulation.json
```

The default is 30 turns; use `--turns 10` for an initial review and longer runs
for targeted investigations. Checkpoints are published every 10 turns.

Reports default to `--report-detail lean`: important adapter changes are written
to the `.changes.jsonl` sidecar, events omit routine repetition, and full snapshots
are retained at checkpoints and finding boundaries. Start with the report's
`findings` and `analytics.metrics`, then follow evidence references. Use
`--report-detail verbose` for full per-turn snapshots and every selected event.
Verbose storage uses lossless deltas and shared payloads. Retrieve complete
records with `cargo run --offline --bin report_read -- report.json --turn 80`
or add `--stream events --sequence N` for an event. Keep report sidecars together.
Both modes provide the same complete analyzer inputs. See
[Headless analytics](docs/HEADLESS_ANALYTICS.md) for change-stream fields and coverage.

To benchmark the pinned Freeciv server with the same map presets, build it
once from the repository root. This requires Meson, Ninja, a C compiler, and
the ICU and zlib development packages.

```sh
git submodule update --init reference/freeciv
meson setup reference/freeciv/build-headless reference/freeciv --buildtype=release \
  -Dclients=[] -Dfcmp=[] -Dtools=[] -Daudio=none \
  -Dnls=false -Dfcdb=[] -Dmwand=false
meson compile -C reference/freeciv/build-headless freeciv-server
```

Then run the 60×60, four-civilization example:

```sh
cargo run --offline --release --bin headless_sim -- \
  --engine freeciv --seed 42 --map-size tiny --turns 200 \
  --report /tmp/freeciv-benchmark.json
```

The Freeciv mode runs Hard AI under `civ2civ3` and disables autosaves. It opens
a loopback listener on an ephemeral port. The JSON report records wall time,
the stop reason, and Freeciv's server and AI CPU timings. A game can end
before the requested turn limit if Freeciv declares a winner. This report has
its own format for native Freeciv timings.

To use an existing build, pass `--freeciv-server PATH`. If the build comes
from a different checkout, set `FREECIV_DATA_PATH` to that checkout's
`data` directory. Freeciv mode accepts `--seed`, `--map-size`, `--turns`,
and `--report` with the same meanings as in the CivCraft command. It uses
Freeciv's native terrain generation and ruleset settings.

Run a registered deterministic scenario or resume a saved campaign through the
same analytics pipeline:

```sh
cargo run --offline --release --bin headless_sim -- --list-scenarios
cargo run --offline --release --bin headless_sim -- \
  --scenario city-healing --turns 10 --report /tmp/healing.json
cargo run --offline --release --bin headless_sim -- \
  --scenario settlement-ferry --turns 1 --report /tmp/ferry.json
cargo run --offline --release --bin headless_sim -- \
  --load-campaign /path/to/campaign.save --turns 10 --report /tmp/resumed.json
```

The default features include scenarios and the desktop test surface. Build the
headless runner with `--no-default-features --features scenarios` to exclude the
desktop dependencies and keep registered scenarios. Omit `--features scenarios`
for a runner that lists only `generated-ai`.

`--scenario` and `--load-campaign` are mutually exclusive. Named scenarios
record their version and setup checks in `run.scenario`; loaded campaigns record
the source path. Both attach a fresh trace collector before simulation begins.

Report schema 8 provides a compact summary and two streamed evidence sidecars:

- `/tmp/civcraft-simulation.json`: run configuration/status, enabled analyzers,
  metrics, coverage, findings, final totals, and a compact timeline.
- `/tmp/civcraft-simulation.json.snapshots.jsonl`: initial state and one typed
  snapshot per completed step, including selected/required city, civilization,
  unit, lifecycle, and research providers. Research records known technologies,
  current targets/progress/costs, science output/rates, and acquisitions/losses
  by source. Technology IDs resolve through the report’s shared `technology_catalog`.
  Lifecycle snapshots track units built, existing,
  destroyed, consumed or disbanded, and cities conquered, transferred or destroyed.
- `/tmp/civcraft-simulation.json.minimap.png`: one final terrain and territory minimap.
  Map arrays and intermediate images are not stored in turn history.
- `/tmp/civcraft-simulation.json.events.jsonl`: structured events carrying turn,
  phase, sequence, participants, and domain-owned payloads.

For performance investigations, pass `--profile-turns true`. The summary then
includes per-turn phase timings, named scope aggregates (call count and
inclusive/exclusive time), entity counts, and operation counters.

Read the summary first, then select evidence by snapshot turn or event sequence:

```sh
jq '.run, .summary, .findings' /tmp/civcraft-simulation.json
jq -c 'select(.turn == 11) | .cities[] | select(.id == 1)' \
  /tmp/civcraft-simulation.json.snapshots.jsonl
```

Analytics are independently registered modules. The initial set includes growth
and settlement heuristics plus an event-only production metric. Use
`--list-analyzers`, `--analytics growth,settlement`, or `--analytics none`.
`--trace-domains`, `--trace-entities`, event-buffer/output limits, and
`--snapshot-providers` control evidence capture. Analyzer requirements are
captured even when their evidence is filtered from output; unavailable event
references and collector gaps are explicit.

Findings are heuristic candidates unless an analyzer explicitly checks an
established invariant. Repeated observations update an episode, explicit recovery
resolves it, and recurrence opens a new episode. By default findings do not stop
the run; `--stop-on-finding any|invariant` opts in. A finding-free run does not
prove gameplay correctness. Current observation boundaries are initial state
and end of turn; phase events preserve transient values from inside the turn.

See [Headless analytics and tracing](docs/HEADLESS_ANALYTICS.md) for extension
interfaces, configuration, evidence semantics, and schema migration details.

## Git hooks

Install the repository's local hooks after cloning:

```sh
./scripts/install-git-hooks.sh
```

The pre-commit hook checks Rust formatting and runs Clippy across all targets,
treating warnings as errors, matching CI. The pre-push hook runs formatting,
Clippy, `cargo test --all-targets`, and doc tests. A failed
check blocks the commit or push. Run `./scripts/check-rust.sh` manually to run
the full pre-push checks.

World-generation controls match the interactive game: map size accepts the
Civ III presets or `WIDTHxHEIGHT`; ocean coverage accepts `60`, `70`, or `80`;
climate accepts `arid`, `normal`, or `wet`; world age accepts `3`, `4`, or `5`;
landform accepts `archipelago`, `continents`, or `pangaea`; and temperature
accepts `cool`, `temperate`, or `warm`.

## Scenario architecture

Registered gameplay checks live in `src/scenarios`: one catalog owns their
stable IDs, scripted actions, turn observations, and named checks. A scenario
can check an immediate action or observe normal campaign turns. Regression
coverage uses this same catalog. `src/headless` executes the selected scenario,
attaches tracing and analytics, advances turns, and publishes reports.
Simulation rules and fixture construction remain owned by `src/sim`.

Private rule assertions remain ordinary Rust unit tests under `src/sim/tests`
and subsystem test modules. Reusable setup lives in `src/sim/fixtures.rs`.
See [scenario boundaries and coverage](docs/SCENARIO_REFACTOR.md) for the
architecture and the helper migration inventory.
