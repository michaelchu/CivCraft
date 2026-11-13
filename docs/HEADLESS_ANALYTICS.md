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
| Scenario inputs | `src/headless/scenario.rs` | Build named fixtures or load persisted campaigns before tracing |

There are two extension paths: events for transitions/decisions and providers
for persistent state. A new analyzer can use either or both. No runner or
reporter changes are needed to register an analyzer or provider.

## Scenario inputs

The runner accepts either `--scenario NAME` or `--load-campaign PATH`. Use
`--list-scenarios` to inspect registered names. Both forms produce a normal
`Simulation`, enable all civilizations as AI, attach a fresh trace collector,
and then enter the same snapshot, analyzer, event, and reporting loop. They are
mutually exclusive; omitting both retains generated-campaign behavior.

Named scenarios are versioned Rust definitions. Their setup checks are written
under `run.scenario` and failed checks make the process exit unsuccessfully
after publishing the final report. Saved campaigns preserve elaborate starting
states without persisting trace buffers or analyzer state. The report records
their source path as provenance.

Reusable state mutation belongs in simulation-level scenario helpers. Scenario
definitions compose setup, actions, and checks; desktop-only input/rendering
assertions stay in the desktop playtest layer. This allows engine scenarios to
migrate out of `--self-test` without coupling the headless runner to UI code.

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
observed behavior, not an expected Civ III rules oracle.

## Add an analyzer

1. Create a module under `src/analytics/analyzers/` and implement `Analyzer`.
2. Return a `Descriptor` containing a unique stable ID, analyzer version,
   event kinds, snapshot provider IDs, and effective thresholds.
3. Add its module declaration and factory entry in `registry.rs` (including
   `IDS`, used by `--list-analyzers`).

The `production` analyzer is an event-only example. It subscribes to
`production.completed`, keeps a counter, and publishes a metric. The `growth`
analyzer shows stateful comparisons across typed snapshots. The `settlement`
analyzer shows periodic checks using multiple providers.

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
- `--trace-domains all|none|DOMAIN,...` selects persisted event domains.
- `--trace-entities all|city:N,unit:N,civilization:N` retains an event if any
  listed participant matches. Filters apply to persisted events, not snapshots
  or analyzer inputs. Arbitrary domain names allow newly instrumented modules;
  an unmatched name produces no events from that domain.
- `--snapshot-providers cities,civilizations,units` selects output providers;
  `none` records just the envelope plus analyzer-required data.
- `--trace-buffer-events 10000` bounds requested events buffered per full turn.
  Overflow increments a counter and notifies analyzers. Payload sizes vary.
- `--trace-max-events 50000` and `--trace-max-bytes 16777216` bound persisted
  events for the whole run; these limits do not stop analyzer dispatch.
- `--stop-on-finding none|any|invariant` optionally stops after a full turn.
  Defaults to `none`; built-in heuristics cannot trigger `invariant` mode.
- `--checkpoint-every 10` controls publication frequency.

Snapshots stream to disk rather than accumulating full history in memory. Their
size grows with requested turns and selected/required providers; event limits
do not limit snapshots. The compact timeline and findings remain in memory.
I/O errors fail the runner rather than silently continuing without evidence.

## Report schema 5

`report.json` is the review entry point:

- `run`: status, stop reason, effective configuration, elapsed execution time,
  commit SHA, dirty-tree indicator, turn count and game-over state.
- `analytics`: enabled analyzer versions, thresholds, observation boundaries,
  and namespaced metrics.
- `coverage`: required-event overflow, events not retained by filters/limits,
  and effective snapshot providers.
- `summary`, `findings`, `timeline`: compact review information.
- `evidence`: relative filenames, record counts, and lookup guidance.

Sidecars:

- `report.json.snapshots.jsonl`: initial snapshot plus one per completed step.
  `cities`, `civilizations`, and `units` are independent provider sections.
- `report.json.events.jsonl`: events selected for persistence. Sequences are
  stable within the run but may have gaps after filtering or overflow.

Finding evidence references use `snapshot_turn` or `event_sequence`. Event
references omitted from disk are marked `available:false`. Snapshots referenced
by the built-in analyzers are always retained. Turn counters in phase events
can precede the following end-of-turn snapshot's counter.

Both streams flush before a summary checkpoint is atomically published.
`running` means a checkpoint; `completed` means the runner reached its turn
limit, game over, or configured finding stop. A terminated run can leave its
last valid `running` checkpoint. No findings is not proof of correct gameplay.

Schema 5 replaces schema 4's history sidecar with independent snapshots/events,
adds the initial snapshot and analyzer metadata, and introduces finding episode
IDs. Readers should inspect `version` before applying schema-specific queries.

## Test entry points

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
