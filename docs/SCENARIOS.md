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

Paired runtimes lend their comparison campaign through
`ScenarioRun::comparison_game`. The headless runner advances the live
comparison once per primary turn, including requested preparatory turns, before
calling the observer. An ended companion stays stopped. Observers collect evidence
and may drain trace events; they never advance a campaign clock. `trait-campaign`
uses this path for its matched Rome/Korea comparison.

Runtimes may inject changed conditions through `before_turn`, called by the runner
before each ordinary turn. This hook does not run during preparatory action turns.

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
`trait-campaign`. The remaining thirteen `Actions` definitions control individual
rule inputs or transitions. They retain `Drive` where they need a rule result or
an explicit phase boundary, rather than scheduling an AI player's activities.

| Definitions | Controlled contract |
| --- | --- |
| `combat-matrix` | Movement-triggered combat results under terrain and city-defense variants. |
| `city-healing` | Unit restoration with and without the relevant healing improvement. |
| `culture-history`, `lifecycle-history`, `research-history` | History/provenance for explicit production, capture, loss, research, treaty and upkeep operations. |
| `great-leader` | Leader consumption and completion of a selected city improvement. |
| `expansion-balance`, `shared-strategy`, `wartime-funding` | Advisor/cache repricing, shared policy weights, purchase quotes and upkeep/reserve constraints under changed inputs. |
| `proactive-trade`, `peaceful-first-contact`, `relationship-memory`, `competing-trade-partners` | Atomic diplomatic choices, valuation, first-contact rules and directional relationship memory. |

`diplomacy-parity` is a runtime with focused actions for treaty expiry and atomic
technology transfer. It requests a preparatory turn from the runner to check a
real begin-phase war declaration. It does not sequence internal AI activities.
The changing-condition runtimes below use narrow `Drive` steps to inject city
loss or move fixture defenders before a normal turn. Every remaining `Drive`
method has a scenario caller; the whole-player AI convenience wrapper is compiled
only for private unit tests.

## AI response to changing conditions

| Scenario | Seed | Turns | What it checks |
| --- | --- | --- | --- |
| `border-reinforcements` | 83 | 75 | Clones the border offensive and deploys two pikemen on a visible approach after the first attacker stages. The pikemen can fall back into the city once war begins. Compares declaration timing, home defense, and eventual capture. |
| `wartime-upkeep-shock` | 42 | 25 | Removes a revenue city on turn 5 while war is active. Checks the income drop and ensuing deficit, upkeep shortfalls, the frontier garrison, and recovery of actual net science. |
| `space-industrial-loss` | 83 | 100 | Removes the highest-output eligible non-capital city building a spaceship part once at least eight parts exist. Checks a surviving city takes on additional work of that part type and launch with the frontier guarded during war. |
| `naval-second-wave` | 42 | 50 | After the first capture, adds reserves on the home shore and an enemy counterattack on the far shore. Tracks the original reserves across the water and checks whether the port holds. |
| `competing-trade-partners` | 42 | 1 | Builds cooperation and grievance through gameplay, equalizes current relations, then submits the same military bid from both partners and a low-risk bid from the partner who broke the pact. |

The runtime scenarios apply fixture changes before the next full turn. In a
paired run the runner advances both campaigns. Read the comparison under
`run.scenario.observations` and individual results under `run.scenario.checks`.
