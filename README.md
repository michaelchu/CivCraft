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
  --seed 42 --map-size tiny --ocean-coverage 70 --climate normal \
  --world-age 4 --landform continents --temperature temperate \
  --turns 30 --checkpoint-every 10 \
  --report /tmp/civcraft-simulation.json
```

The default is 30 turns; use `--turns 10` for an initial review and longer runs
for targeted investigations. Checkpoints are published every 10 turns.

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

`--scenario` and `--load-campaign` are mutually exclusive. Named scenarios
record their version and setup checks in `run.scenario`; loaded campaigns record
the source path. Both attach a fresh trace collector before simulation begins.

Report schema 6 provides a compact summary and two streamed evidence sidecars:

- `/tmp/civcraft-simulation.json`: run configuration/status, enabled analyzers,
  metrics, coverage, findings, final totals, and a compact timeline.
- `/tmp/civcraft-simulation.json.snapshots.jsonl`: initial state and one typed
  snapshot per completed step, including selected/required city, civilization,
  and unit providers.
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

World-generation controls match the interactive game: map size accepts the
Civ III presets or `WIDTHxHEIGHT`; ocean coverage accepts `60`, `70`, or `80`;
climate accepts `arid`, `normal`, or `wet`; world age accepts `3`, `4`, or `5`;
landform accepts `archipelago`, `continents`, or `pangaea`; and temperature
accepts `cool`, `temperate`, or `warm`.
