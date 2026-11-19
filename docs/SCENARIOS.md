# Scenario architecture

## Ownership

- **`src/scenarios`** owns registered gameplay scenarios. `catalog.rs` combines the
  always-available `generated-ai` entry with the optional `definitions/mod.rs` table.
  Each definition owns its setup composition, actions, observations and named checks.
- **`src/headless`** (the runner) owns tracing, analytics, turn advancement and report
  output. Scenario code never advances a full turn itself.
- **`src/sim`** owns rules, state, typed fixtures and read-only queries. It does not
  import the catalog or any scenario definition.
- **`src/client`** owns rendering, input and screenshot capture. UI review's simulation
  setup lives in `sim/ui_review`.

## Lifecycle and definitions

`ScenarioRun` is the common lifecycle for scripted actions and normal-turn
observations. A script may request preparatory turns; the runner executes them inside
the action boundary, which keeps each scenario's version, checks and report provenance
stable.

Every definition uses `Scenario::new(id, version, kind)`. `ScenarioKind` selects one
generated run, a deferred action script, or a prepared runtime, so actions and runtime
setup cannot both be configured. `Control` and `RuntimeWorld` are explicit enum values.
Report versions are independent per-scenario contracts, not constructor defaults.
Scenario IDs, registration order, check names and versions are stable CLI and report
contracts; change them deliberately.

All definitions share one gate, `cfg(any(test, feature = "scenarios"))`, on
`scenarios::definitions`. The catalog, lifecycle and `generated-ai` remain available
without the `scenarios` feature.

## Fixtures and driving the simulation

- **`sim/fixtures`** holds typed state preparation, split by domain (`world`, `cities`,
  `units`, `diplomacy`, `economy`, `research`, `readiness`); `fixtures.rs` owns only the
  builder lifecycle. Blank terrain presets are reusable fixtures. Fixed rosters,
  coordinates, research weights and funding arrangements live with the scenario that
  chooses them.
- **`sim::drive::Drive`** is the only way scenarios call state-mutating rule steps
  (import it explicitly). Keep the underlying `Simulation` methods `pub(in crate::sim)`
  and add a wrapper in `sim/drive.rs` rather than widening them to `pub(crate)`.
  Read-only queries may stay `pub(crate)`. Prefer `Command` or a normal turn when a
  scenario can use one.

## Tests

There are two Rust test boundaries. Source-adjacent `tests` modules own unit tests
with private access (compiled only under `cfg(test)`, no registry or runner); the root
`tests/` directory owns executable/CLI contract tests.

- Shared simulation checks enter through `sim::tests::gameplay` (`sim/tests/gameplay`).
- Subsystem helpers live in each module's `tests/helpers.rs`.
- Setup helpers use fixture names; other test helpers use test names.
- Private RNG vectors, arithmetic, caches and list-order assertions stay as unit tests
  rather than becoming public scenario API.
- A unit test that merely calls an already registered scenario is redundant: the
  catalog test (`scripted_scenarios_pass_on_the_reference_campaign`) already runs every
  scripted scenario.

See [`HEADLESS_ANALYTICS.md`](HEADLESS_ANALYTICS.md) for running scenarios and reading
reports.
