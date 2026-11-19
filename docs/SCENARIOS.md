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

Paired runtimes lend their companion campaigns through
`ScenarioRun::companion_campaigns`. The headless runner advances each live
companion once per primary turn, including requested preparatory turns, before
calling the observer. An ended companion stays stopped. Observers collect evidence
and may drain trace events; they never advance a campaign clock. `trait-campaign`
uses this path for its matched Rome/Korea comparison.

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

`settlement-ferry` is a normal-turn example: version 2 watches AI-selected ferry
and founder production, transport of that exact founder, and island founding
within 16 turns. Use `--scenario settlement-ferry --seed 42 --turns 16`.

CI builds `civcraft` with `--no-default-features --features desktop-audio` to keep
the desktop build working without the scenario catalog, fixtures or `Drive`.
Default feature selection is unchanged.

`ai-paradrops` version 2 is another normal-turn workflow: run it for two turns.
It checks airborne reinforcement before conquest and preservation of the home
guard, with no combat-dependent outcome. The broad `Drive::run_ai_player` and
`Drive::run_ai_paratrooper` wrappers are removed; focused landing-rule checks
retain private access in the aviation unit tests.

## Scripted-rule audit

Full workflows use normal turns, including `ai-paradrops` and the paired
`trait-campaign`. The remaining twelve `Actions` definitions control individual
rule inputs or transitions. They retain `Drive` where they need a rule result or
an explicit phase boundary, rather than scheduling an AI player's activities.

| Definitions | Controlled contract |
| --- | --- |
| `combat-matrix` | Movement-triggered combat results under terrain and city-defense variants. |
| `city-healing` | Unit restoration with and without the relevant healing improvement. |
| `culture-history`, `lifecycle-history`, `research-history` | History/provenance for explicit production, capture, loss, research, treaty and upkeep operations. |
| `great-leader` | Leader consumption and completion of a selected city improvement. |
| `expansion-balance`, `shared-strategy`, `wartime-funding` | Advisor/cache repricing, shared policy weights, purchase quotes and upkeep/reserve constraints under changed inputs. |
| `proactive-trade`, `peaceful-first-contact`, `relationship-memory` | Atomic diplomatic choices, valuation, first-contact rules and directional relationship memory. |

`diplomacy-parity` is a runtime with focused actions for treaty expiry and atomic
technology transfer. It requests a preparatory turn from the runner to check a
real begin-phase war declaration. It does not sequence internal AI activities.
Every remaining `Drive` method has a caller in this set; the whole-player AI
convenience wrapper is compiled only for private unit tests.
