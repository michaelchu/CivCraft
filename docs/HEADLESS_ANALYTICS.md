# Headless analytics and tracing

The headless runner advances the real simulation. Domain modules emit passive
observations; independent analyzers interpret them; the reporter streams evidence
and publishes a compact review summary. Analytics never receive mutable gameplay
state and never participate in gameplay decisions.

## Components

| Component | Location | Responsibility |
| --- | --- | --- |
| CLI and orchestration | `src/headless_sim.rs`, `src/lib.rs`, `src/headless/` | Configuration, advancing turns, dispatch, checkpoints |
| Runtime collector and event envelope | `src/sim/trace/` | Lazy capture, subscriptions, phase labels, sequence IDs, overflow accounting |
| Domain event payloads | `src/sim/cities/observations.rs`, `src/sim/settlement/observations.rs` | Typed, versioned facts about actual execution |
| Read-only adapters and typed snapshots | `src/sim/observation.rs` | Expose observational state without JSON strings or mutation |
| Snapshot provider registry | `src/analytics/providers.rs` | Capture requested domains once per observation boundary |
| Analyzer interface and registry | `src/analytics/mod.rs`, `src/analytics/registry.rs` | Select independent modules and union their requirements |
| Analyzers | `src/analytics/analyzers/` | State, thresholds, findings, metrics |
| Finding lifecycle | `src/analytics/finding.rs` | Episodes, recovery, recurrence, evidence references |
| Output | `src/headless/report.rs` | Stream JSONL and atomically replace summary checkpoints |
| Scenario inputs | `src/headless/scenario.rs`, `src/scenarios/` | Prepare registered scenarios or load persisted campaigns, then run traced actions |

There are two extension paths: events for transitions/decisions and providers
for persistent state. A new analyzer can use either or both. No runner or
reporter changes are needed to register an analyzer or provider.

## Scenario inputs

The runner accepts either `--scenario NAME` or `--load-campaign PATH`. Use
`--list-scenarios` to inspect registered names. Both forms produce a normal
`Simulation` and enter the same snapshot, analyzer, event, and reporting
loop. The runner attaches tracing before scripted actions and enables all
civilizations as AI before advancing turns. They are
mutually exclusive; omitting both retains generated-campaign behavior.

Named scenarios are versioned Rust definitions. Their checks are written
under `run.scenario` and failed checks make the process exit unsuccessfully
after publishing the final report. Saved campaigns preserve elaborate starting
states without persisting trace buffers or analyzer state. The report records
their source path as provenance.

The lifecycle is `prepare -> attach tracing/start analyzers -> execute actions
-> capture scenario_actions boundary -> simulate turns -> finish checks`.
Scenario actions therefore emit ordinary retained events and produce a typed
snapshot before turn simulation begins. Generated campaigns and loaded saves
without scripted actions omit that extra boundary.

Typed fixture construction belongs in the domain modules under `src/sim/fixtures/`.
Registered definitions and checks belong in `src/scenarios/definitions`. Scenario
definitions compose setup, actions, and checks; desktop-only input/rendering
assertions belong in the visual review path without coupling the headless
runner to UI code.

### Registering a scenario

The catalog entry point is `src/scenarios/catalog.rs`. It includes `generated-ai`
and the optional definitions registered in `src/scenarios/definitions/mod.rs`.
Direct definitions live in `src/scenarios/definitions`. A scripted definition is
a function over a newly generated campaign that records named checks:

```rust
pub(super) const SCENARIO: Scenario = Scenario::new(
    "example-action",
    1, // Explicit report contract version; independent of execution form.
    ScenarioKind::Actions { control: Control::Human, execute },
);

fn execute(game: &mut Simulation, checks: &mut Checks) {
    // Prepare through WorldBuilder, execute an engine action, then record checks.
    checks.push(("example_outcome", passed, detail));
}
```

Registration makes it available as `--scenario example-action` with no runner
change. `Scenario::new(id, version, kind)` is the only constructor. The enum keeps
mutually exclusive execution forms separate, and versions are explicit per
registered definition. `Control::Human` preserves the human seat before actions;
`Control::AllAi` enables every seat first. Command scripts enable all-AI play after
actions complete. Scripted actions use the reference map generator for the
requested seed and report that map's effective settings. The
`generated-ai` scenario accepts generation options. Fixed scenarios and
loaded campaigns reject world setup flags. Saved campaigns report their
seed and dimensions and leave generator settings empty.

For a scenario that spans normal campaign turns, use `ScenarioKind::Runtime`
with `control`, `world` and `setup` fields. `RuntimeWorld::Custom` describes a
fixture-built world and omits generator metadata. `RuntimeWorld::Reference`
retains reference-map provenance, as used by the scripted diplomacy runtime.
The setup receives the generated map and returns a starting campaign and an
observer for that run. Use the shared world builder to construct fixtures and
keep the returned city and unit IDs in the observer. Setup runs before the
initial snapshot. The observer receives each completed turn and cannot mutate
the campaign. Its structured observations appear under
`run.scenario.observations`. At every normal stop, including early game over
or a short turn budget, the runner asks the observer to evaluate all required
outcomes. Only those assertions contribute to the scenario check totals.

A runtime with `Control::Human` keeps the human seat active while the other
civilizations run normally. Scenarios can request trace kinds needed for their
checks independently of the report's event filters.

`ScenarioRun` is the per-run lifecycle for actions, observations and final
checks. Scripted actions can request preparatory turns with `action_turns` and
inspect them in `complete_actions`; the runner executes those turns within the
action boundary. The runner uses it for both scripted and normal-turn scenarios.

The world builder takes an ordered list of `CivPreset` entries. Each entry
uses a stable catalog ID and optionally replaces the civilization's starting
technologies. `None` uses the catalog defaults, and `Some(&[])` starts with
no technologies. The order assigns active player slots starting at zero.
The blank sea map retains the caller's seed and topology. Set the calendar
before placing entities or setting treaty history, then call `finish()`
before handing the campaign to the runner.

```rust
let mut game = WorldBuilder::ocean_campaign(
    map,
    &[
        CivPreset { id: "england", technologies: Some(&[Tech::COMBUSTION]) },
        CivPreset { id: "germany", technologies: None },
    ],
);
let mut world = WorldBuilder::new(&mut game);
world.start_at(120, 1900);
world.grassland((10, 24));
let city = world.city(0, (10, 24), 3);
let ship = world.unit(1, UnitKind::Battleship, (10, 25));
world.diplomacy(0, 1, Pact::Peace, 42, [3000, 5000]);
world.finish();
```

The builder also supports `government(owner, government)` for an unlocked
starting government, `embassy(holder, host)` and
`shared_vision(giver, receiver)` for directional access between contacted
civilizations, and `trade_route(from_city_id, to_city_id, goods)` for an
established route. Set governments, cities, and diplomacy before adding
routes. Routes must be legal for their goods, endpoints, and current pact,
and both cities must have a free route slot. Presetting a route does not pay
a delivery bonus. Normal AI and route rules apply after setup.

Run `--scenario campaign-state-presets --seed 42 --turns 2` to see the four
presets in action. The check verifies the starting governments at setup and
tracks diplomatic access, live sight of a remote capital, and route income
at both endpoints over two campaign turns. The report also shows later
government changes made by the AI.

Use `world.tile(position, TilePreset::new(terrain, base_terrain))` to lay out
a fixture map before placing cities and units. A tile preset can also specify a
resource, river, road, railroad, or military base. A railroad includes a road,
and the builder rebuilds reciprocal river edges when the map changes.
`world.city_improvement(city_id, building)` places a regular improvement that
satisfies the city's normal construction requirements.

Run `--scenario harbor-trade-route --seed 42 --turns 1` to check that two
coastal cities with harbors start without a trade route and establish one
during normal turn processing. A third coastal city without a harbor serves
as a control. The report shows the route's goods and income at both ports.
The building catalog marks Harbor as infrastructure so the AI accounts for
its trade network role before deciding which improvements to sell.

Use `city_production` to set a legal current build, its stored shields, and
an ordered worklist. Future worklist items are checked when reached.
`city_workforce` sets worked tiles and specialist counts in scientist, taxman,
entertainer order. Set the city's territory before assigning its workers.
`load_cargo` uses the regular boarding rules for units on the same tile.
Set `unit_readiness` after boarding or changing rank and HP. It presets
remaining movement fragments, fuel, and activity state.
`economy` sets treasury and science, tax, and luxury rates under the current
government. `research` sets an available technology, accumulated bulbs, and
an optional reachable goal.

Run `--scenario campaign-readiness-presets --seed 42 --turns 1` to check a
defended city with a worklist, a loaded Carrier and fortified guard, a
treasury and tax split, and a project one bulb from completion. The first
normal turn completes the Worker, advances the Harbor from the worklist,
and discovers Alphabet.

The naval assault is defined in
`src/scenarios/definitions/naval/ai_assault.rs`. Run it with
`--scenario naval-ai-assault --seed 42 --turns 30`. It starts with a blank sea
map and an established war, then watches the original Galleon and its troops.
The short run path is covered in the CLI integration tests. The registry unit
test checks scenario names and exercises the scripted cases.

The ship combat scenario in `src/scenarios/definitions/naval/ship_combat.rs` uses the
same API. Run it with `--scenario naval-ship-combat --seed 42 --turns 1`.
An elite Submarine intercepts a damaged Battleship on open water. The
observer tracks both ships by ID and requires a combat kill credited to
the Submarine's civilization and survival of the original Submarine.

The reference checks use seed 42. `settlement-ferry` version 2 uses fixed Rome/Inca
slots, an isolated coastal home and a fertile overseas island. Run it with
`--scenario settlement-ferry --seed 42 --turns 16`. The AI selects and funds
production through normal turns; the observer checks `ferry_queued`,
`settler_sequenced` and `island_founded`. It tracks the home-built ferry and founder
by ID, requires that founder aboard the ferry at sea, and matches its founding
removal to the new island city. Outcomes must occur within 16 completed turns;
a run that stops before completing the milestones publishes unmet checks and fails.
Setup runs before the initial snapshot, with no scripted action boundary or
mid-run fixture mutations. Generator metadata is omitted for this custom world.
Seeds 1 and 18, which exposed roster-dependent failures in the old script, now
use the same controlled roster and geometry. The normal-turn test also repeats
seed 42 to check deterministic observations.

`ai-paradrops` version 2 runs two normal turns with fixed Rome/Inca identities.
Run `--scenario ai-paradrops --seed 42 --turns 2`. An airborne unit must choose
an empty threatened city over an available undefended enemy city, while
the original guard stays at home. Islands isolate the choices, a spotter provides
vision of the approach, and the distant threat avoids a combat roll after landing.
The observer tracks exact unit IDs and latches the three required outcomes within
two turns. Setup precedes tracing; no actions or fixture edits run after it.
The old version-1 seed-3 failure used the generated roster and manually invoked
an AI phase. Defended-landing refusal and vacant-city capture remain focused
aviation unit checks, alongside this normal-turn workflow.

### City governor campaign comparison

`city-governor-campaign` founds two human cities on the generated map and
advances the campaign with no further player commands. The paired
`city-governor-control` starts with the same citizen assignments and releases
governor control before turn one. Use the same seed and turn budget for both.
The control measures what happens when existing assignments are left alone,
including the simulation's ordinary assignment reconciliation.

```sh
cargo build --offline --release --bin headless_sim
mkdir -p /tmp/civcraft-governor
for seed in 42 101 202 303 404; do
  case "$seed" in
    42|202|404) turns=85 ;;
    *) turns=35 ;;
  esac
  for scenario in city-governor-campaign city-governor-control; do
    target/release/headless_sim --scenario "$scenario" --seed "$seed" \
      --turns "$turns" --analytics none --trace-domains none \
      --report "/tmp/civcraft-governor/$scenario-$seed.json"
  done
done
```

To test recovery directly, run `city-governor-food-shock` and
`city-governor-food-shock-control` for one turn. Both start with the capital
working tiles that have just lost their food yields. Replacement tiles
remain available. The governed arm must apply nonnegative food in the
first city turn, and the control must apply a deficit. This checks the
actual city result after reassignment.

To test unrest handling, run `city-governor-unrest-shock` and
`city-governor-unrest-shock-control` with the same seed for one turn. Both
start with two size-five human cities on grassland. The report records whether
the target city forecasts disorder before turn processing and whether any city
enters disorder after processing. The governed case must start with a disorder
risk and finish without disorder; the control must exhibit disorder. This
isolates citizen assignment's effect on city mood, without changing empire tax
rates or adding military police.

The governed run fails its intervention check if a city never activates,
hands control back, has a food deficit or disorder, starves, or is destroyed
by famine. A failed check still writes a report. Check coverage and city
retention before comparing outcomes. Use each report's final
`run.scenario.observations[-1].state.totals` for accumulated counts. Growth
events record food applied during city processing. Each surviving city's
disorder flag comes from the disorder update in that phase. Output forecasts
describe the assignments at the end of each turn and list food, shields,
trade, gold, luxury, and science in that order. The reports distinguish those
forecasts from the food actually applied.

This comparison measures unattended citizen assignment during the opening
campaign. Production queues, military moves, empire policy, and a human
player's possible interventions are outside the experiment.

## Add a game trace

Define a serializable payload in the owning domain. Give it a stable namespaced
kind and increment its payload version for incompatible changes:

```rust
#[derive(serde::Serialize)]
struct UpkeepCharged {
    gold_before: i32,
    gold_after: i32,
    upkeep: i32,
}
impl crate::sim::trace::Payload for UpkeepCharged {
    const KIND: &'static str = "economy.upkeep_charged";
    const VERSION: u16 = 1;
}
```

Emit at the point where the result is known, using values the game already
computed. Both closures are lazy; disabled, unsubscribed, or full collectors do
not construct their arguments:

```rust
self.trace.emit(
    self.turn,
    || vec![trace::Entity::Civilization(owner)],
    || UpkeepCharged { gold_before, gold_after, upkeep },
);
```

Do not recompute a decision, draw random numbers, or change gameplay for tracing.
Report the reason actually used by a decision; do not infer one afterward.
Include all relevant participants so entity filters preserve interactions.
`emit_caused` accepts an optional prior event sequence when a direct causal link
is known. Existing initial hooks do not infer causal relationships.

The collector derives the source domain from the kind prefix. New domains do
not need registration in the collector. Prefix subscriptions (`economy.*`) and
analyzer subscriptions to exact kinds are supported. An event can be decoded
with `Event::decode::<PayloadType>()` when the payload also implements
`Deserialize`; the helper checks its kind and schema version.

Currently instrumented: actual city food/growth processing, production
completion, city founding, and settlement-site selection. Growth traces describe
observed behavior, not an expected Freeciv rules oracle.

## Government and diplomacy evidence

The default analyzers now include `government` and `diplomacy`. Their independent
snapshot providers are `governments` and `diplomacy`; requesting either analyzer
retains its required snapshots even with `--snapshot-providers none`.

`governments` records each configured nation's name, current government, pending revolution,
alive/AI status, city count, candidate availability and cached scores, best
available choice, research goal, and evaluation/refresh turns. Missing advice
has null scores and evaluation dates. Observation never populates or refreshes
the advisor. Scores can be stale between the normal advisor refreshes; they
measure the engine's heuristic, not proven long-term optimality.

`government.decision` records the pre-decision advice, selected government and
outcome (`retained`, `revolution_started`, `rejected`, or `deferred`). Deferred
means an existing revolution or no cities prevented a selection.
`government.changed` separately records revolution starts (including war-driven
revolutions) and actual adoptions. The government analyzer counts decisions and
changes, flags selections that differ from the published best available advice,
and flags end-of-turn government/advice differences with no pending revolution.
These are candidates for investigation; inspect score age and availability.

`diplomacy` records sorted player pairs with names and alive flags, first contact, pacts and expiry turns,
directional trust, embassies, shared vision, pending offers, and war plans with
countdowns and reasons. Directional arrays follow `players` order. A null pact
means no contact, distinct from explicit War.
Regular civilizations intentionally start first contact at Peace with low trust
(the owner-approved Civ III-style exception); barbarians remain at War.
Run `--scenario peaceful-first-contact --turns 1` to check peaceful contact,
trade-driven trust, explicit war declarations and barbarian hostility.

`diplomacy.transition` records contact, treaty changes and expiry, AI offer
proposals, treaty execution acceptance/rejection, explicit offer declines, war
plans, war decisions, Senate deferrals, and declarations. It includes actor,
counterparty, reason, prior/current pact and applicable treaty terms, including
AI-to-AI interactions. Rejected execution identifies validation/valuation
failure; it does not provide a full breakdown of every rejected clause or every
candidate the AI considered. The diplomacy analyzer counts actions and reports
contacted pairs and active war plans; these describe activity, not diplomatic
optimality. `diplomacy.active_war_plans` counts nonnegative preparation
countdowns between living, contacted players who are not already at war.
Relationship rows retain all stored war-plan state, including negative cooldowns,
the inactive `-1` sentinel, and zero countdowns left after a ceasefire expires
into war. Those entries are not upcoming wars and do not count as active plans.
Pact proposals still exclude the selected nonallied potential enemy, but an
ally is never excluded by that ranking. Economic trade scans peaceful partners
independently, with a persisted per-pair cooldown and a bounded candidate search.
`diplomacy.trade_decision` records candidate counts, rejection categories,
reserves and the selected offer's gains for both sides. The named metrics
`diplomacy.trade_decisions` and `diplomacy.recent_trade_decisions` summarize these
decisions. Lean mode suppresses identical repeated unsuccessful decisions;
verbose mode retains them all. Use `--scenario proactive-trade --turns 1` for
focused coverage. The policy is intentionally independent of Freeciv's AI;
parameters and trait interests are in `content/diplomacy.toml`.
Directional `relationship_memory` rows expose decayed commerce, loyalty and
grievance scores, their last credited actions, and derived attitude/trade effects.
`diplomacy.relationship_memory` events record completed mutual technology
exchanges, actual support of an ally's existing war and unjustified pact breaches.
The analyzer publishes reason counts in `diplomacy.relationship_memories` and
the latest eight events in `diplomacy.recent_relationship_memories`. Counts are
directional: a completed bilateral trade normally credits both partners.
Cooldowns and caps limit repeated credit; passive decay creates no event.
Use the release `relationship-memory` scenario for focused verification.
Needs-based gold, map and recovery proposals emit
`diplomacy.negotiation_decision`; reason counts and the latest eight decisions
appear under `diplomacy.negotiation_decisions` and
`diplomacy.recent_negotiation_decisions`. Decisions include proposed terms,
the actor's reserve, funding shortfall and recovery values. A selected proposal
is an intention: follow its `offer_accepted`, `offer_rejected` or pending human
offer evidence to establish execution. Map values count previously unknown
transferable tiles, so repeated maps do not have recurring information value.
Aid is bounded by verified need, trust, donor surplus and a per-offer cap.
All these proposals share the persisted pair cadence with technology trade.
Lean mode suppresses identical unsuccessful negotiation decisions independently
of technology decisions; verbose retains them all. Run the release
`diplomatic-aid`, `diplomatic-maps`, and `diplomatic-recovery` scenarios with
`--seed 42 --turns 1`. Their fixtures precede the initial snapshot, and checks
match selected terms to actual accepted treaties during normal advisor turns.
Government and diplomacy event counts are run-local and marked
incomplete after collector overflow. Output filters do not filter analyzer input.

For a focused report:

```sh
cargo run --offline --release --bin headless_sim -- \
  --turns 80 --analytics government,diplomacy \
  --snapshot-providers governments,diplomacy --report /tmp/politics.json
```

Start inspection in `report.json`, before opening the raw streams:

- `findings` contains candidates for investigation and their evidence references.
- `analytics.metrics["government.nations"]` names the surviving nations and shows
  current/best available governments, research goals, pending revolutions, advice
  age and the snapshot turn to inspect for full candidate scores.
- `government.decision_outcomes` counts every collected decision, including those
  compacted out of event output. `government.recent_changes` shows the latest
  32 government changes with evidence references.
- `diplomacy.relationships` names contacted pairs and shows pacts, trust, pending
  offers and war plans. `diplomacy.recent_activity` shows the latest 32 actions
  with reasons and evidence references. These recent lists are bounded views;
  earlier retained events remain in the event sidecar.
- Event evidence includes `available:false` when output filters or limits omit
  its record. `coverage.events_compacted_by_kind` separately counts compaction;
  `coverage.events_not_retained` includes compaction, filters and output limits.

The default `--report-detail lean` mode applies across adapters. It writes
`report.json.changes.jsonl`: per-boundary records containing only changed entity
rows, with `provider`, stable `key`, `before`, and `after`. Null `before` means
creation; null `after` means removal. The first record establishes the lean
baseline. Rows are inspection projections, not complete state or a replay log:

| Provider | Lean inspection content |
| --- | --- |
| Cities | Population, ownership, production, disorder, specialists, growth status; omits routine food counters and worked-tile arrays |
| Civilizations | Identity, survival, city/unit counts; omits routine gold/score ticks |
| Units | Identity, owner, type, HP, orders/state, worker task; omits movement coordinates/budgets, route destination and work countdown |
| Research | Known technology, research target/goal, rates and discoveries/losses; omits progress/accounting ticks |
| Lifecycle | Changed cumulative totals and every boundary-local lifecycle fact |
| Governments | Adoption, revolution target, available governments, best choice and research goal; detailed score refreshes stay in retained decision events |
| Diplomacy | Contact, pact, offers, embassies/vision, war-plan reason/status and trust bands of 1,000; omits countdown and small trust ticks |
| Territory/custom providers | Changed full values; unknown fields are preserved |

Research and lifecycle boundary-local changes are always appended when nonempty,
even if consecutive batches match. Government/diplomacy snapshots omit unused
civilization slots, retain configured nations after elimination, and preserve IDs.

Lean mode persists full snapshots at the initial boundary, scenario actions,
checkpoints, finding evidence boundaries and the final boundary. The timeline
lists these retained boundaries. All analyzer inputs remain complete each turn.
This lets readers follow a finding to a full snapshot without keeping every
routine dump. The final report's research/lifecycle summaries retain full totals.

Lean event output suppresses identical successful government decisions until
advice/state changes, ordinary food-storage ticks that exactly match nonnegative surplus without
population change, and successful settlement-search diagnostic counters. It preserves
refreshes, anomalies, growth/starvation, failed searches, government changes,
production, settlement choices/founding, research, lifecycle and diplomacy events.
Compaction happens only when writing output; analyzers still receive every event.

Use `--report-detail verbose` to preserve every exact snapshot and selected event
(ordinary trace filters and limits still apply). Storage is lossless: verbose
snapshots use field/array deltas between full baselines, and large event payload
members share dictionary records. No per-turn values, decisions, entity details,
array order or interval history are discarded. The changes stream stays empty.
Profiling is a separate opt-in measurement of computation cost.

Expand evidence before using ordinary JSON tools:

```sh
cargo run --offline --bin report_read -- report.json --turn 80
cargo run --offline --bin report_read -- report.json --stream events --sequence 476
cargo run --offline --bin report_read -- report.json --stream snapshots > expanded.snapshots.jsonl
```

`report_read` supports raw lean/older JSONL and verbose encoded streams. Optional
`--turn`, `--phase` and event `--sequence` filters apply after reconstruction.
Missing baselines, sequence gaps, malformed edits or unresolved shared references
fail explicitly. The reader honors published record counts so unfinished tails
from a running simulation do not invalidate its last checkpoint. Keep the report
and its sidecars together; individual verbose
lines can depend on earlier records or the shared-values sidecar.

## Opt-in simulation profiling

With `--profile-turns true`, simulation code can create a scoped timing with a
stable module/operation name:

```rust
let profiler = self.trace.profiler();
let _scope = profiler.scope("ai.worker_planning");
self.plan_workers();
```

The cloned handle lets a scope stay alive while the simulation is mutably
borrowed. Scopes close automatically, including on early return, and may nest.
Profiles aggregate call count, inclusive time, and exclusive time by name for
each turn. Disabled profiling returns no scope and does not time the operation.
Prefer meaningful work units over one scope per tiny helper; pair timings with
existing or new `count_operation` workload counters when investigating scaling.
Nested inclusive timings overlap, so use exclusive time when comparing
contributions across scopes; do not sum inclusive timings.

Focused scopes include `worker.plan` and `worker.route_map`,
`offense.target_search`, `government.score_evaluation`,
`city_manager.arrange`, and `city_manager.query`. The arrangement scope includes
lattice construction and nested search calls; compare its exclusive time with
the nested query time to separate setup from search. Worker routing records
expanded map nodes plus eligible and reachable worksite candidates, so those
counts show the remaining search work relative to the destinations the planner
can use. Use the existing worker route, offensive search, government
evaluation, and city-manager iteration counters alongside the new measurements.

AI ramp investigations also have `ai.units.*` branch timings and
`offense.candidate_route`. `movement.route_costs_to_many` measures shared
destination searches; the `movement.route_search_graph_multi_*` counters show
their calls and expanded nodes. Ground invasion planning records
`ai.invasion_support.batch_calls` and `ai.invasion_support.units_scanned` for
its per-attacker city-marker pass. In Freeciv's `ai/default/daiunit.c`,
`find_something_to_kill()` initializes invasion markers once per attacker and
uses one pathfinder map to score multiple cities. Keep that shape when adding
target scoring so a new city does not trigger another full route traversal.

Settlement profiling separates `settlement.best_city_site`,
`settlement.ferry.rendezvous`, `settlement.ferry.route`,
`settlement.ferry.best_plan`, `settlement.ferry.dispatch`, and
`settlement.ferry.pickup_route_search` per rendezvous, nested under
`advisor.build.settlement` or `ai.transports_and_settlement`. Compare exclusive
times to avoid double-counting nested scopes. Per-turn counters report site
search expansions and candidate evaluations, ferry pickup candidates,
destinations and searches per rendezvous, eligible ships and founders,
sea/land route expansions, and generic route-search calls and expansions.
`movement.route_costs_to_many` reports a shared multi-destination traversal;
compare its map expansions with the number of pickup destinations queried.

## Add an analyzer

1. Create a module under `src/analytics/analyzers/` and implement `Analyzer`.
2. Return a `Descriptor` containing a unique stable ID, analyzer version,
   event kinds, snapshot provider IDs, and effective thresholds.
3. Add its module declaration and factory entry in `registry.rs` (including
   `IDS`, used by `--list-analyzers`).

The `production` analyzer is an event-only example. It subscribes to
`production.completed`, keeps a counter, and publishes a metric. The `growth`
analyzer shows stateful comparisons across typed snapshots. The `settlement`
analyzer shows periodic checks using multiple providers. Its
`expansion_by_civilization` metric compares Settlers produced, cities founded,
active founder-capable units, selected sites, and viable site candidates found
across searches. Candidate totals count repeated geographic sites again when
separate searches revisit them; they are search activity, not a count of unique
available locations. `settlement.site_search_completed` reports candidate
counts and expanded tiles for each active Settler's assignment search; virtual
searches used only to rank city production choices are not included in those
per-civilization search counters.

Callbacks:

- `on_start`: initialize against the snapshot before the first simulated turn.
- `on_event`: consume subscribed events in deterministic emission order.
- `on_boundary`: inspect an end-of-turn snapshot.
- `on_gap`: learn how many requested events the collector dropped in a batch.
- `finish`: publish final metrics without resolving unobserved conditions.

Analyzer IDs namespace metrics and finding keys. Use `AnalysisOutput::check`
for a heuristic candidate and `invariant` only for an established invariant.
Pass a stable per-condition/entity key. Explicit negative checks resolve an
existing episode; missing data or silence never resolves it. A later recurrence
opens a new finding ID. First/last observations do not prove continuity across
unobserved intervals. The current built-in checks remain heuristic candidates.

After an event gap, event-based negative checks cannot close findings during
that run; an explicit complete snapshot check can still establish recovery.
Event-dependent analyzers should handle `on_gap` to invalidate their own counters
or inference state. Global coverage metadata also marks event-derived results
as incomplete. Filtering output alone does not create a gap in analyzer input.

Thresholds for the initial modules are exposed through `--growth-stall-turns`
and `--settlement-check-every`. Add configuration for a new analyzer in the CLI
configuration/factory, while leaving the execution loop unchanged.

## Add a snapshot provider

Implement `SnapshotProvider` in the provider registry (or a module it registers).
Its read-only `capture(&Simulation, &mut Snapshot)` method inserts typed data
under the provider's stable ID. Register the provider and its selectable ID in
`providers.rs`; no central enum or snapshot schema edit is required. Use a
read-only domain adapter on `Simulation` if the needed state is private.

```rust
// In the provider:
snapshot.insert("economy", EconomySnapshot { /* observed values */ });

// In an analyzer declaring the "economy" provider:
let economy = snapshot.get::<EconomySnapshot>("economy");
```

Snapshot values must implement `Serialize + 'static`. Typed access uses a
checked downcast, not JSON parsing. Provider IDs must not collide with the
reserved envelope fields `turn`, `phase`, and `city_count`, or other providers.
The registry captures the union of analyzer requirements and requested output
providers once per boundary. Required data is retained even if omitted from
`--snapshot-providers`.

City `growth_status` describes the current forecast and size/production limits;
actual failure to accumulate food is assessed by the growth analyzer.

The initial boundaries are `initial` and `turn_end`. Events carry finer phase
labels (`end_activities`, `city_processing`, `world_events`, `unit_activities`,
`ai_planning`, `turn_end`) and are drained after the full turn. There are no
callbacks inside gameplay and no intermediate-phase state snapshots yet.
Events must carry any transient before/after values needed for analysis.

## Configuration and coverage

```sh
cargo run --offline --release --bin headless_sim -- \
  --seed 42 --turns 10 --report /tmp/review.json \
  --analytics growth,settlement,production \
  --trace-domains cities,production,settlement \
  --trace-entities city:1,civilization:0
```

- `--analytics none` disables analyzers; `--list-analyzers` lists registered IDs.
- `--report-detail lean|verbose` selects output detail across adapters. Lean is
  the default. It does not change analyzer input or gameplay.
- `--trace-domains all|none|DOMAIN,...` selects persisted event domains.
- `--trace-entities all|city:N,unit:N,civilization:N` retains an event if any
  listed participant matches. Filters apply to persisted events, not snapshots
  or analyzer inputs. Arbitrary domain names allow newly instrumented modules;
  an unmatched name produces no events from that domain.
- `--snapshot-providers cities,civilizations,units,lifecycle,research,governments,diplomacy,minimap` selects output providers
  (these eight are the default; `territory` is also available). The final minimap
  requires the default `desktop` feature; graphics-free builds omit it;
  `none` records just the envelope plus analyzer-required data.
- The `territory` provider records map dimensions, row-major tile ownership,
  and a land mask so unowned land can be distinguished from water. City
  snapshots include centers; settlement events record selected-site score,
  travel and nearest-city distances, radius overlap, first-worker tile, and
  actual founding distance to the nearest existing city.
- `--trace-buffer-events 10000` bounds requested events buffered per full turn.
  Overflow increments a counter and notifies analyzers. Payload sizes vary.
  Zero dropped events establishes collector coverage only; it does not prove
  every gameplay path emits an event. Diplomatic contact is initialized by
  physical contact or a contact-granting effect; visibility alone does not
  establish diplomatic contact.
- `--trace-max-events 50000` and `--trace-max-bytes 16777216` bound persisted
  events for the whole run; these limits do not stop analyzer dispatch.
- `--stop-on-finding none|any|invariant` optionally stops after a full turn.
  Defaults to `none`; built-in heuristics cannot trigger `invariant` mode.
- `--checkpoint-every 10` controls publication frequency.
- `--profile-turns timing` records lightweight total/phase wall-clock timings
  and starting city/unit counts, without named scopes or operation counters.
  Use this mode to observe turn-time growth. The default is `false` (off).
- `--profile-turns true` retains full detailed profiling for existing scripts.
  Add `--profile-prefixes worker,city_manager` to collect only matching
  scope and counter names (exact names and dot-separated descendants, not
  arbitrary string prefixes). Names are filtered independently: selecting
  `ai` does not include counters named `normal_paths`, for example. Unknown
  but syntactically valid prefixes collect nothing; inspect an unfiltered
  report for available names. Excluded scopes are rejected before locking.
- `--profile-start-turn N --profile-end-turn N` restrict detailed profiling to
  an inclusive **campaign turn** range, including when loading a save. Either
  bound may be omitted. Total/phase timings remain available on every turn.
  These filters require `--profile-turns true`; they do not filter analytics
  events or snapshots. Each profile's `details_recorded` identifies whether
  details were enabled for that turn. Empty details do not imply no work.
  With prefix filters, exclusive scope time subtracts only recorded child
  scopes; unrecorded child work remains attributed to the recorded parent.
- Leave profiling disabled for ordinary scenario checks and benchmark final
  speed with it disabled. Detailed scopes add measurable overhead; compare
  fixed turn windows with the same selection. Report configuration records
  `profile_mode`, `profile_prefixes`, and the selected campaign-turn bounds.

For example, observe timings cheaply, or inspect worker planning at turns 100–110:

```sh
cargo run --offline --release --bin headless_sim -- --seed 42 --turns 200 \
  --profile-turns timing --report /tmp/turn-timing.json
cargo run --offline --release --bin headless_sim -- --seed 42 --turns 110 \
  --profile-turns true --profile-prefixes worker \
  --profile-start-turn 100 --profile-end-turn 110 --report /tmp/worker-profile.json
```

Snapshots stream to disk rather than accumulating full history in memory. Their
size grows with requested turns and selected/required providers; event limits
do not limit snapshots. The compact timeline and findings remain in memory.
I/O errors fail the runner rather than silently continuing without evidence.

## Report schema 10

`report.json` is the review entry point:

- `run`: status, stop reason, effective configuration, elapsed execution time,
  commit SHA, dirty-tree indicator, turn count and game-over state.
  `configuration.map_generator` identifies the generator. `world_settings`
  contains the selected generator's inputs and is null for a loaded campaign.
- `analytics`: enabled analyzer versions, thresholds, observation boundaries,
  and namespaced metrics.
- `coverage`: required-event overflow, events not retained by filters/limits,
  and effective snapshot providers.
- `summary`, `findings`, `timeline`: compact review information.
- `turn_profiles`: present as an empty array unless profiling is enabled. Each
  entry contains total turn time, phase times, scoped timing aggregates
  (`spans`, with call count and inclusive/exclusive seconds), starting entity
  counts, and operation counts such as city-manager search iterations and
  advisor cache hits/misses. Timings are diagnostic wall-clock observations
  and do not participate in simulation decisions.
- `evidence`: relative filenames, record counts, and lookup guidance.

Sidecars:

- `report.json.changes.jsonl`: lean entity changes and interval history; see the
  inspection projections above. Empty in verbose mode.
- `report.json.snapshots.jsonl`: full snapshots at retained boundaries in lean
  mode. Verbose uses `jsonl_snapshot_delta_v1`: an initial/full baseline and exact
  deltas for every completed step. Full baselines occur at least every 64 records
  (or sooner when a full record is smaller). Use `report_read` for full JSONL.
  `cities`, `civilizations`, `units`, `lifecycle`, `research`, `governments`,
  and `diplomacy` are independent provider sections.
- `report.json.minimap.png`: one final minimap, written when the run stops.
  It shows terrain, current territory and city markers without fog of war, using
  the in-game terrain/civilization palette and staggered, horizontally wrapped
  projection. `evidence.minimap` links the PNG and records its turn, phase,
  map dimensions and civilization color legend. This is an asset-free simulation
  overview, not a screenshot of the desktop UI.
  The default `minimap` output selection is final-only: no map arrays are written
  to per-turn snapshots, no intermediate PNGs or HTML viewer are generated, and
  running checkpoints have `evidence.minimap:null`. Excluding `minimap` from
  `--snapshot-providers` disables the export; `none` does so too. Builds using
  `--no-default-features --features scenarios` retain research analytics but omit
  minimap support and keep `evidence.minimap:null`.
- `report.json.events.jsonl`: selected events, raw JSONL in lean mode or
  `jsonl_events_shared_v1` wrappers in verbose mode. Every selected verbose event
  has a record; sequence gaps can still arise from filtering or overflow.
- `report.json.shared.jsonl`: verbose event payload members of at least 128
  serialized bytes that are objects/arrays, stored once and referenced by ID.
  Empty in lean mode. The event byte limit includes both event and shared-value
  bytes; rejected/filtered events do not publish new dictionary entries.

Finding evidence references use `snapshot_turn` or `event_sequence`. Event
references omitted from disk are marked `available:false`. Snapshots referenced
by the built-in analyzers are always retained. Turn counters in phase events
can precede the following end-of-turn snapshot's counter.

All streams flush before a summary checkpoint is atomically published.
`running` means a checkpoint; `completed` means the runner reached its turn
limit, game over, or configured finding stop. A terminated run can leave its
last valid `running` checkpoint. No findings is not proof of correct gameplay.

Schema 10 adds lossless verbose encoding and `report_read`. Inspect each
stream's `evidence.*.format` before parsing; encoded lines are not standalone
snapshots/events. Schema 9 adds lean/verbose output, the inspection change stream,
sparse full snapshot retention, and separate compaction accounting. Schema 8 adds
lifecycle accounting and each live unit's `home_city`. Schema 6
replaces implicit sequential step timings with scoped timing
aggregates. Schema 5 replaced schema 4's history sidecar with independent
snapshots/events, added the initial snapshot and analyzer metadata, and
introduced finding episode IDs. Readers should inspect `version` before
applying schema-specific queries.

For ramp analysis, use one fixed seed and world configuration, then compare
the mean and median of turns 1–10 against turns 71–80, the worst individual
turn, and the phase/scope shares. Record the late/early ratio explicitly.
Operation counts help distinguish algorithmic growth from machine noise. Always
compare snapshot sidecars byte-for-byte when evaluating an optimization.

Run from turn zero when measuring the end-to-end ramp or checking that the
slowdown naturally recurs. For focused late-game profiling, load a saved
campaign from around turn 70 and profile the next ten turns. This skips setup,
but represents only that saved state's population, unit rosters, and AI
decisions. Prefer a naturally advanced save over a hand-built turn-80 scenario
when diagnosing the observed slowdown. Use a named scenario for a compact,
controlled workload that exercises one specific path.

### Research analytics

The default `research` provider records each civilization's sorted
`known_technology_ids`, `known_count`, `future_techs`, `current_technology_id`,
`goal_technology_id`, current `progress`/`cost`, and `science_rate_percent`.
Resolve technology IDs using the report's single `technology_catalog` (ID, name,
era and prerequisites). Names and prerequisites are not repeated in every snapshot.
Unset targets/goals have null IDs; progress can be negative during a science deficit.
Barbarians and unused civilization slots are excluded from the civilization rows.

`science_output` records the last actual accounting phase: its turn, gross science,
technology upkeep and net output. It is null before the first observed accounting
phase. An inactive civilization can retain an older output record; inspect its turn.
This records the upkeep actually charged, rather than recalculating it after new
technologies have changed maintenance costs.

`changes` contains acquisitions and losses since the previous snapshot, with turn,
phase, technology ID, owner, source and other owner where applicable. Sources
include `research`, `trade` (including treaty gifts), `espionage`, `conquest`,
`village`, `free_advance`, `first_discovery_bonus`, `civil_war`, `upkeep` and
`transfer_loss`. Test-fixture grants without a more specific source use `unspecified`.
A technology gained and lost within one turn retains both records. Future Tech
completions and losses count individually and also update `future_techs`.

Each civilization's `totals` records initial knowledge, cumulative acquisitions
and losses, and counts by source. The latest state, totals and last interval are
also under `summary.research`; read `research.changes` batches in the lean change
stream or verbose snapshots for earlier intervals.
History begins at `since_turn`, so starting or loaded technologies are baseline
knowledge, not new discoveries. Saves and speculative clones do not carry this
run-local history. Disabled analyzers, trace filters and event-buffer/output limits
do not truncate analyzer inputs or research history in the lean change stream
(or verbose snapshots). Direct uninstrumented knowledge edits are
marked `unobserved` and set `complete_since_start:false` rather than guessing a source.

```sh
jq '.summary.research.civilizations[] | {name, known_count, science_rate_percent, science_output, totals}' report.json
jq -c '.changes[] | select(.provider == "research.changes") | .events[]' report.json.changes.jsonl
```

Run `--scenario research-history --turns 1` to exercise actual research completion,
treaty transfer and upkeep-driven loss with four regression checks. Select
`--snapshot-providers research` for research-only data, or exclude `research` to
omit it and the shared catalog. The final-only `minimap` export is independent.

### Unit and city lifecycle history

The default `lifecycle` provider adds cumulative `unit_totals` by owner and unit
kind, cumulative `city_totals` by owner, and `changes` since the previous snapshot.
The final totals and final interval also appear at `summary.lifecycle`. Read the
lean change stream (`provider:"lifecycle.changes"`) or verbose snapshot stream
for the full history; each interval is written once to avoid repeating an
ever-growing history in every snapshot.

Unit totals distinguish city production (`built`, a subset of `created`), live
units (`existing`), destruction, consumption (founding/joining a city or an
expendable action), disbanding, transfers, and upgrades. Change records identify
the unit, type, owner, location, turn and phase, with a reason or creation source.
Built units include the producing `origin.city_id`; `units[].home_city` is the
current support city and may differ. Transfers that replace a unit ID link the
previous ID. Upgrades and transfers do not count as new production or deaths.
A Great Leader created by an elite unit's victory has `origin.source` `great_leader`
and the winning unit's ID in `origin.winner_id`; it counts as `created`, not `built`.

City records identify the city ID, name, location, population and owner before
the action. Conquests record both owners; trade, propaganda and civil-war
transfers have separate reasons. Destruction records its reason and responsible
owner when known. A city captured and then destroyed between snapshots retains
both records, as does a unit built and lost within one interval.
A size-one city razed during conquest records destruction with reason `conquest`;
it does not count as an ownership transfer.

History starts at `since_turn`. Starting units and cities, including those in a
loaded campaign, count as `initial`; earlier production or losses cannot be
reconstructed from a save. Cloning or saving a campaign does not carry its
run-local history. Event filters, disabled analyzers and event-buffer/output
limits do not truncate lifecycle snapshots. Unexpected uninstrumented roster
changes are explicitly marked `unobserved`, counted separately, and set
`complete_since_start:false` rather than inventing causes.

```sh
jq '.summary.lifecycle.unit_totals, .summary.lifecycle.city_totals' report.json
cargo run --offline --bin report_read -- report.json --stream snapshots | \
  jq -c '.lifecycle.changes[] | select(.event == "city_transferred" or .event == "city_destroyed")'
```

Run `--scenario lifecycle-history --turns 1` for a regression exercising actual
production, same-interval loss, founding, disbanding, conquest, trade and city
destruction.

Run `--scenario culture-borders --turns 1` to verify accumulated city culture
claims a tile after crossing a border tier. Run
`--scenario culture-wonder-capture --turns 1` to check audited Palace and Sistine
Chapel rates, 1,000-year doubling, capture output, and original-builder
recapture. Run `--scenario culture-empire-victory --turns 1` to exercise the
map-scaled culture target and two-to-one lead through normal turn processing,
with culture distributed below the legendary-city threshold.

### Seed-42 80-turn ramp baseline

The deterministic tiny-map workload below is the current ramp regression:

```sh
cargo run --offline --release --bin headless_sim -- \
  --seed 42 --map-size tiny \
  --turns 80 --checkpoint-every 10 --profile-turns true \
  --report /tmp/civcraft-ramp80.json
```

Summarize one run, or emit an array of summaries for directly comparable runs:

```sh
cargo run --offline --release --bin ramp_report -- \
  /tmp/civcraft-ramp80-before.json /tmp/civcraft-ramp80-after.json
```

The tool uses the first and last ten recorded profiles and reports their mean
and median time, mean entity counts, the late/early mean ratio, worst turn,
and early/late phase, per-operation span timing/call, and operation means. Keep
the workload and host fixed when comparing wall-clock values. Operation counts
are the preferred evidence that an optimization changed scaling rather than
host noise.

On the profiling host used for the November 2026 optimization pass, the final
run averaged 0.674 seconds for turns 1–10 and 7.468 seconds for turns 71–80,
an 11.08x late/early ratio. The worst turn was 11.35 seconds. Host timing is
noisy; operation counts and byte-identical snapshot sidecars are the stronger
comparison evidence. The aspirational ratio remains 3x or less and has not
been reached.

The remaining growth is distributed rather than attributable to one fixed
overhead:

- Offensive research still performs destination-specific path searches per
  advised city. Target searches grew from 2.1 to 15.5 per turn and candidate
  routes from zero to 79; these searches cannot be combined naively because
  movement class, boarding, hostile endpoints, and city origin affect costs.
- AI unit planning scales with the live unit roster and includes defense,
  reinforcement, invasion, and per-unit movement searches.
- Worker planning constructs a route map per eligible real or virtual worker
  and values activities across owned city radii. Activity-tile lookups are
  indexed, but route construction and tile valuation remain per worker.
- Government reevaluation periodically runs bounded city-manager searches for
  each candidate government. This produces the remaining five-turn spikes;
  staggering evaluations would change AI decisions and is not a transparent
  optimization.
- City production, domestic technology advice, wonders, and emergency worker
  assignment scale at least linearly with city count. Some contain nested
  building, technology, or city-manager candidate loops.

Do not share destination-specific path maps or retain advisor results across
turns without proving their invalidation rules. Production choices, unit
positions, diplomacy, visibility, transport state, technology, and city
assignments can all change the result. Prefer operation-count reductions with
exact snapshot parity over improvements visible only in one wall-clock run.

### Freeciv-guided ramp pass

The pinned Freeciv source at commit `3efab1385e` supplies the architecture for
the next pass. In particular, `ai/default/daicity.c` schedules slow-changing
building and founder wants over five to nine turns, while continuing to
evaluate volatile military and worker advice every turn;
`server/advisors/infracache.c` computes city-tile improvement values once per
worker phase; `common/aicore/citymap.c` uses flat map-indexed phase storage;
and the `pf_map_move_costs_iterate` users in `ai/default/daiunit.c` stop lazy
path expansion when the remaining movement-cost horizon cannot win.

CivCraft now follows those shapes without introducing a global path cache:

- stable building and settlement advice is persisted per city, scheduled over
  deterministic five-to-nine-turn intervals, and invalidated by production
  completion, capture, technology changes, and government changes;
- worker tile/activity values and safety checks are shared within a phase,
  while route cost, reservations, work rate, and road-network bonuses remain
  worker-specific; worker activity occupancy uses map-indexed vectors;
- production advice shares infrastructure values and compatible terrain
  projections across the owner's virtual workers, following
  `daicity.c:dai_manage_cities`'s single `initialize_infrastructure_cache`
  before the city loop. This cache is local to `prepare_ai_city_defenses`;
  isolated advice starts fresh, and nothing survives into spending, movement,
  city processing, or the next pass. Routes and arrival times are still
  calculated separately for each virtual worker;
- worker planning skips units without the worker flag, matching `daisettler.c`'s
  `UTYF_WORKERS` guard. Road-to-rail eligibility shares the phase's terrain
  projection cache. Road-network bonuses are reused only while the actual
  road/rail worker activity maps remain unchanged, and founding a city clears
  these caches along with other terrain projections. This preserves
  `infracache.c`'s requirement that newly assigned road work affect bonuses;
- government evaluation scans owner and home-city unit facts once per refresh;
- ferry and rendezvous searches consume `NormalPaths` lazily and stop after a
  result is determined. Cross-actor reuse remains deliberately unsupported
  because movement, visibility, diplomacy, transport, risk, and zone of
  control are part of path compatibility.

The stable-advice schedule deliberately adopts Freeciv's less-opportunistic
five-to-nine-turn behavior, so its scenario snapshots can differ from the old
every-turn advisor. The schedule is saved to preserve deterministic reloads.
Military emergencies are not cached.

On the same quiet November 2026 host, clean release runs at commits `66663d0`
and `114ba0a` measured:

| metric | before | after | change |
| --- | ---: | ---: | ---: |
| turns 1-10 mean | 0.647 s | 0.667 s | +3.1% |
| turns 71-80 mean | 7.519 s | 6.608 s | -12.1% |
| late/early ratio | 11.62x | 9.90x | -14.8% |
| worst turn | 12.18 s | 11.71 s | -3.9% |

The pass materially reduced the ramp but did not reach the aspirational 3x
ratio. The remaining late window is led by domestic and military technology
advice, government projection, worker routing, and per-unit AI planning. Use
`ramp_report` and the new advisor, worker, government, and path operation
counters for subsequent comparisons.

## Test entry points

### Expansion balance

The `expansion-balance` scenario checks that founder backlogs and other cities'
founder queues immediately reduce production wants even when site advice is
cached, that clearing those commitments restores expansion, and that valuable
opportunities remain possible beyond twenty cities.

CivCraft deliberately tunes Freeciv's dense small-city preference: overlap is
more expensive and unclaimed room for growth is worth more. Founder production
also pays a rising price against the smaller of government administration
capacity (`Empire_Size_Base`) and a fair share of habitable map area. Live
founders and other cities' founder queues count as committed expansion. The
ruleset's `preferred_land_tiles_per_city` and `expansion_cost_priority` tune
this price. `free_expansion_cities` preserves an initial foothold, including
the first ferry on a small island. This changes AI incentives, not founding
legality or a city cap.
These are CivCraft balance choices, not claims about unmodified Freeciv AI.

```sh
cargo run --offline --release --bin headless_sim -- \
  --scenario expansion-balance --turns 1 --report /tmp/expansion-balance.json
```

Both executables are thin wrappers around the library. Shared simulation,
analytics, UI, and CLI tests run once in the library harness:

```sh
cargo test --offline --lib -- analytics:: headless:: sim::trace::
cargo test --offline --no-fail-fast -- --test-threads=4
```

The binaries have `test = false`; select `--lib` rather than `--bin headless_sim`
for focused simulator tests. The headless invocation and report schema are
unchanged.

## Verification requirements

- Compare tracing on/off using the same seed, inputs, and observation providers;
  require identical serialized campaign and gameplay RNG state.
- Verify trace state is absent from saves and resets on load and cloning.
- Test detector thresholds, explicit recovery, recurrence, and missing data.
- Test subscriptions, entity filters, lazy disabled capture, overflow, and
  output caps; distinguish analytical gaps from evidence omissions.
- Verify summary references resolve to flushed evidence or are marked unavailable.
- Exercise a registered event-only analyzer and a custom typed provider to
  ensure the runner and snapshot envelope need no domain-specific changes.


### Shared AI strategic priorities

Civilization snapshots include `strategy` for living AI nations (null for human
or defeated nations). Weights for economy, science and military are relative to
100, not budget percentages. `posture` is `develop`, `recover`, `defend`, `prepare`
or `space_race`; crisis, war count, preparation and space-program flags expose
the reasons. `projected_income` and `gold_reserve` are retained in full snapshots
but omitted from lean changes to avoid routine financial noise. Strategy is a
read-only current-state assessment, not a persisted commitment or a prediction
of victory. Trait weights and pressure bonuses live in `content/strategy.toml`;
both research and trade use the technology interests in `content/diplomacy.toml`.

For example, list posture changes from a lean report:

```sh
jq -c '.turn as $turn | .changes[]
  | select(.provider == "civilizations" and .after.strategy != null)
  | select(.before.strategy.posture != .after.strategy.posture)
  | {turn: $turn, nation: .key, strategy: .after.strategy}' report.json.changes.jsonl
```

Run `--scenario shared-strategy --turns 1` in release mode for controlled
research/trade interest, wartime priority, reserve and recovery checks.


Run `--scenario trait-campaign --seed 42 --turns 20` for a paired runtime
comparison. The scenario clones the initial campaign and changes the subject's
civilization from Rome (Commercial/Militaristic) to Korea
(Commercial/Scientific). Starting assets, opponents, and RNG state are shared.
The report's `run.scenario.observations[].state` contains both campaigns' city
queues, production completions, rush purchases, research, accepted technology
trades, and border-war objectives. The scenario checks that Rome starts preparing
before Korea in this matchup. Compare the other outcomes in the observations.
Both campaigns are observed through the same turn, up to the requested turn
limit or an earlier game-over in the Rome campaign.
The report's general analytics and sidecars describe the Rome campaign.

### AI funding evidence

`economy.funding_decision` records native advice and selected `[science, tax,
luxury]` rates, the saved-rate income, reserve/target, concrete military purchase
costs, required net income and the selected-rate forecast. Normal cash targets
refill over `strategy.toml`'s `reserve_refill_turns`; `upkeep_buffer_turns` keeps
operating cash behind discretionary purchases and trade. Routine preparation
purchases include the cash floor enforced by spending. Only eligible selected
purchases are quoted; an undefended city in grave danger can request the defense
advisor's replacement. Discretionary refills retain a science share based on the
common science versus economic/military priorities. Urgent tax funding can waive
that share only when the next accounting phase can actually fund the purchase;
ordinary upkeep can still take priority to avoid a deficit. War status alone is not a funding need.

These forecasts use current workers, specialists, building modifiers, mood and
unit upkeep, including homeless units. The operating cushion also covers the
higher unit bill in anarchy or an available government, so a regime with free
support cannot spend its transition cash on a routine upgrade. Emergency
spending retains its existing override. Later growth, production, citizen
rearrangement, combat or discoveries can change actual accounting. `target_met`
means the selected rates meet the current forecast, not that an unaffordable
purchase can be bought immediately. A search that cannot improve income keeps
the original rates. Refilling discretionary reserves cannot create research
debt; imminent treasury default or urgent defense retains the ability to
prioritize survival.

Lean compaction keeps changes in selected rates, reason or target satisfaction;
verbose mode retains every collected decision and its financial inputs. Both
modes retain every `economy.upkeep_shortfall` event: `building_sale`,
`unit_upkeep_action`, or `treasury_writeoff`. These are important losses, not
ordinary voluntary sales/upgrades. Events identify the nation and the affected
city/building or unit where available. Debt settlement mechanics are unchanged.
Include `economy` in trace domains when narrowing a run's coverage.

```sh
cargo run --offline --bin report_read -- report.json --stream events |
  jq -c 'select(.kind == "economy.funding_decision"
               or .kind == "economy.upkeep_shortfall")
         | {turn, phase, sequence, kind, payload}'
```

Use the `wartime-funding` release scenario for a controlled policy regression;
use verbose evidence for per-turn funding/crisis counts. Compacted lean events
cannot establish those frequencies without their omitted observations.

### Vassal diplomacy evidence

Diplomacy analyzer version 7 adds `vassalage` and `independence_reason` to
`analytics.metrics.diplomacy.relationships`. The agreement identifies its
subject, master, kind, start turn and original land total. Formation, eligibility
and dissolution use the existing `diplomacy.transition` event and appear in
`recent_activity` with evidence references. An eligibility event is not proof
of departure: inspect `vassalage_ended` and the next relationship snapshot.

## Religion observations

The `religion` snapshot provider is enabled by default and may also be selected
explicitly. It records religion identities, each civilization's declared faith,
policy/change turn, and each city's influence, dominant faith and unaligned
weight. Shares use `influence / (unaligned_weight + sum(influence))`.
`religion.change` events record actual founding, declaration, first-presence and
conversion transitions with civilization/city entity references. Diplomatic
snapshots include a directional `religion_attitude` pair independently of
relationship memory. These diagnostic snapshots are omniscient; the interactive
client applies its own visibility restrictions.

Religion AI decisions use `religion.ai_decision`. Inspect `economy`, `science`,
`culture`, `founding`, `declared`, `policy`, `previous_score`, `selected_score`,
`research_candidate`, `research_interest`, `actual_research` and
`research_blocked`. The candidate is an optional opportunity, not evidence that
research switched: corroborate it with the normal research decision stream.

Policy decisions also expose `previous_declared`, `previous_policy`,
`previous_components`, `selected_components`, `best_declared`, `best_policy`,
`best_components`, `required_improvement` and `missionary_opportunity`. Component
scores separate domestic alignment, culture, actual contentment relief, local
defense, marginal spread and public diplomacy. The best candidate can be rejected
by the cooldown or the improvement threshold; only the selected state is an
action. Missionary opportunity values additional incoming pressure after the
cap, using all own sources and visibility-safe foreign connections. Devout mood counterfactuals use the complete native citizen pipeline; a potential released
entertainer must remain out of disorder. Switching requires the base improvement
plus `ai.switching_city_cost` per evaluated city.

## Military analytics

The `military` analyzer is registered in the default `--analytics` list and can
be selected on its own (`--analytics military --trace-domains military`). It
tracks passive `military.*` events together with `civilizations`, `diplomacy`,
and `lifecycle` snapshots without mutating gameplay state or adding cost when
disabled.

Events emitted under domain `military`:

- `military.combat`: battle participants, roles, elite/Army status, pre/post HP,
  power, city capture, Cruise Missile expenditure, and Great Leader eligibility,
  ineligibility reason, roll denominator, and created Leader ID.
- `military.leader`: Great Leader roll outcomes (`spawned`, `roll_missed`),
  rushed buildings (with shields saved and whether forming an Army was legal),
  Army formation (`formed_army`), Army member loading (`army_member_loaded`),
  and leader travel/sheltering.
- `military.bombard`: ranged and precision bombardments by artillery, ships,
  missiles, and aircraft (target kind, interceptions, HP damage, kills,
  buildings/population/terrain destroyed).
- `military.air_mission`: aircraft mission breakdown (`bombard`, `air_strike`,
  `air_superiority`, `rebase_bombard`, `rebase_strategic`, `refuel_wait`,
  `refuel_return`, `idle_on_base`), air-superiority interceptions, paratrooper
  drops, and hunter Cruise Missile launches.
- `military.nuclear_strike`: ICBM / Tactical Nuke launches, SDI interceptions,
  unit casualties, cities hit, and fallout tiles.
- `military.production_decision`: AI city production switches and wartime
  military suppressions (`upkeep_constrained`, `ferry_cap`, `space_plan`,
  `wonder_city`, `domestic_outbid`), including the winning military advisor
  stage (`defense`, `bodyguard`, `aircraft`, `offensive`, `hunter`,
  `coordinated_muster`, `training_building`).
- `military.turn_summary`: one summary per AI civilization per turn with
  unlocked military roles, fielded counts by role, per-unit dispatch branch
  counts, wartime idle military counts, coordinated war objective readiness,
  assault transport dispatches, and live Army status (loaded members, capacity,
  dispatch, movement).

Report metrics under `analytics.metrics`:

- `military.nations`: per-civilization scorecard covering unlocked vs built/fielded
  roles (`unlocked_but_never_used_in_war`), production stage wins and suppressions,
  unit-turn dispatch shares, combat efficiency and city softening, bombardment,
  air/missile/nuclear totals, Great Leader eligibility/expected-vs-actual spawns,
  and Army utilization/combat.
- `military.wars`: per-war episode rollups (start/end turns, combats, bombards,
  air/nuclear strikes, city captures, shield exchanges, Army engagements).
- `military.totals`, `military.recent_combats`, `military.recent_strikes`,
  `military.recent_leader_events`: run-wide totals and bounded recent evidence.

Built-in heuristic findings (`heuristic_candidate`) flag `war_army_idle`,
`war_objective_stalled`, `unlocked_role_unused_in_war`, `army_underutilized`,
and `leader_spawn_shortfall`. Lean mode compacts unchanged non-switching
production suppressions, peacetime turn summaries, and repeated idle/sheltered
aircraft or leader records while keeping all combat, strike, leader spawn/use,
and wartime summary events; analyzer metrics are identical in lean and verbose
modes.

```sh
jq '.analytics.metrics["military.nations"][] | {
  name, war_turns, unlocked_but_never_used_in_war,
  combat, bombard, air_and_missiles, leaders_and_armies
}' report.json

jq '.analytics.metrics["military.wars"]' report.json
```
