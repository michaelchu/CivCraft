# CivCraft agent guidance

## Gameplay authority

CivCraft is a Rust implementation of Freeciv gameplay, using selected Civ III
content and artwork. The authority split is:

- **Gameplay logic** (rules, formulas, state transitions, turn processing,
  AI behavior, and rule interpretation) follows the pinned Freeciv source
  (`reference/freeciv`) and its `civ2civ3` ruleset, as copied into the
  `src/rules/engine_data/` layer and the `src/rules/engine_rules/campaign.rs`
  constants. Civ III game logic is no longer an authority for mechanics.
- **Civ III content and artwork** are retained only where the port explicitly
  maps them into the Freeciv model. The installed Conquests data and checked-in
  native catalogs provide identity and asset provenance; they are not gameplay
  authority.
- **Terrain, resources, and other art or asset mappings** stay with the Civ III
  sources where the port plan retains them.

Use [`docs/FREECIV_GAMEPLAY_PORT_PLAN.md`](docs/FREECIV_GAMEPLAY_PORT_PLAN.md)
and the pinned checkout under `reference/freeciv` for implementation comparisons.
When Freeciv is ambiguous, preserve the ambiguity or resolve it from the pinned
Freeciv source and ruleset; do not infer gameplay behavior from Civ III manuals,
legacy code, or community claims.

### Recorded exception: Great Leaders (Civ III)

Leader creation and use follow Civ III, as an explicit project-owner decision:
an elite land unit's kill can create a Great Leader, which can then complete a
city's current improvement or wonder. This is not a Freeciv mechanic, and the
Leader is not a `GameLoss` unit. The design is in
[`docs/superpowers/specs/2026-11-17-great-leader-design.md`](docs/superpowers/specs/2026-11-17-great-leader-design.md);
the odds are community-documented for Civ III patch 1.29f and live in
`data/rulesets/civcraft/content/settlements.toml`. On 2026-11-20 the owner also
authorized consuming a Great Leader in an owned city to form an Army, with one
Army per four owned cities. Armies cannot be produced or queued by cities,
including cities with a Military Academy, as an explicit owner preference.
The Pentagon requires three living owned Armies; it has no artificial era gate.
Army combat uses loaded land combat troops, a combined hitpoint pool, and
member-specific attack/defense through the existing combat modifiers. Loaded
members stay in formation; defeat destroys the whole Army. The Pentagon adds
a fourth slot to its owner's Armies. The slowest member plus one sets movement;
healing, bombardment, presentation and saves preserve the member health pool.
Combat playback shows each selected member's own sprite, health and ordinary
battle clips/sounds. The carrier remains the map anchor; victory plays only
when the complete battle ends.
Keep the existing AI preference for rushing buildings. Validate with
`leader-army`, `army-combat`, `great-leader`, and the source-adjacent Army tests. Keep other
mechanics on the Freeciv authority above.

### Recorded exception: core culture (Civ III Conquests)

The owner authorized a Civ III Conquests culture model on 2026-11-19. Culture
is a core CivCraft strategy and victory path, not a selectable victory flag.
Use cumulative city culture, the audited `culture_per_turn` values in native
building data, a per-building age bonus after 1,000 campaign years, threshold-
based city border growth, and both single-city and empire-wide cultural
victories. Captured buildings produce no culture for an occupier; an original
builder resumes production after recapture without the age bonus. Border
shape and contests are Civ III, not Freeciv: a new city claims a 3×3 square and
each threshold grows it into the "fat X" (21, 45, 77 and 117 tiles); city size,
Freeciv border strength, incumbent retention and the permanent-radius lock do
not apply. A tile goes to the nearest city by culture distance, then the city
with more culture, then the older city, so the shared edge is the line of equal
distance. Land claims are also Civ III: a border covers every tile in its
shape, land or sea and seen or not, with no C2C3 `Tile_Claimable` restrictions
and no `[borders]` ruleset settings. CivCraft topology remains the adapter;
culture flipping is deferred pending probability tuning and clear player-facing
warnings. Keep the design and implementation in
`docs/superpowers/specs/2026-11-19-culture-core-design.md`.

### Recorded exception: custom espionage

The owner confirmed on 2026-11-18 that the existing custom espionage system is
intentional and must be preserved. Its player-to-player embassy/Spy assets,
mission costs, risk tiers and AI use remain in `src/sim/diplomacy/espionage.rs`.
Other diplomatic treaties, map/city clauses and war mechanics still follow
Freeciv, except for first contact and AI policy below. Do not replace custom espionage with
Freeciv Diplomat/Spy units as part of diplomacy parity work.

### Recorded exception: peaceful first contact (Civ III-style)

The owner requested on 2026-11-18 that regular civilizations meet at Peace,
with low initial trust rather than automatic War. Apply this equally to human
and AI nations; a common ally is not required. First contact does not grant an
alliance, embassy, shared vision or a trust bonus beyond the existing small AI
attitude variation. Trade and friendly relations build trust through the
existing diplomacy system. Barbarians and other `No_Diplomacy` players remain
hostile. Preserve established relationships when contact is refreshed or a
campaign is loaded; do not reset an existing war to peace.

### Recorded exception: proactive diplomacy policy

The owner authorized diplomacy AI to depart from Freeciv policy on 2026-11-18.
Economic trade may consider peaceful nonallies and must not filter out an ally
as a potential war target. Technology purchases, sales and swaps use a shared
valuation for proposing and accepting offers, incorporating verified Conquests
traits, research priorities, treasury reserves and military/victory export risk.
Policy parameters and trait interests live in
`data/rulesets/civcraft/content/diplomacy.toml`. Treaty legality, atomic execution,
technology prerequisites/transfer and the custom espionage system remain under
their existing rules. Preserve deterministic decisions, bounded searches and
human approval of incoming offers. Do not infer missing civ traits from names.

The same owner-authorized AI policy now includes shared strategic priorities
across research, trade, tax reserves, routine spending and optional war planning.
Weights live in `content/strategy.toml`; technology interests remain shared in
`content/diplomacy.toml`. Preserve advisor legality, research switching costs,
emergency defense and ally obligations. Economic recovery may postpone optional
wars, and an actual space program can favor science; traits are preferences,
not guaranteed victory routes or mechanical bonuses. Read each civilization's
`strategy` observation and lean posture changes before escalating to verbose
financial inputs. Validate policy changes with `shared-strategy` and
`proactive-trade` release scenarios.

Military funding must follow concrete purchase needs and reserve shortfalls,
not maximize tax for the duration of a war. `strategy.toml` owns the upkeep
buffer and reserve refill horizon shared by taxes, routine spending and trade.
Retain operating cash through routine purchases/upgrades, including support
cost increases during anarchy or an available government. Keep the native tax
calculation as the starting point; bounded forecasts may
raise tax to cover actual upkeep and refill cash, while protecting research
upkeep except during an imminent default or a fundable urgent defense. Ordinary
reserve/purchase refills preserve a science share from the common priorities;
quote only selected purchases or genuine emergency replacements. Validate with the
`wartime-funding` release scenario. Inspect `economy.funding_decision` and
`economy.upkeep_shortfall` evidence: a nonnegative treasury alone does not prove
that no forced sale, disband or debt writeoff occurred. Lean output retains
funding policy/rate/outcome changes and every actual upkeep loss; use verbose
mode when exact per-turn funding inputs are needed.

The owner-authorized diplomacy policy also includes bounded directional memory
of completed technology exchanges, unjustified pact breaches and actual ally
war support. Pending offers, gifts and war plans do not earn cooperation.
Memory decays and affects AI attitudes, trade margins, sensitive exports,
alliance caution and optional wars. Supporting an existing ally's war is not an
unjustified betrayal, even when it ends a ceasefire. Treaty legality, Senate
checks and mandatory military obligations retain their existing rules.
Validate with the `relationship-memory` release scenario. Inspect
`diplomacy.relationship_memories`, `diplomacy.recent_relationship_memories` and
each pair's directional `relationship_memory`; use event evidence to distinguish
a completed action from an intention. Parameters live in `diplomacy.toml`.

Needs-based negotiation extends that AI policy to bounded allied gold assistance,
useful map exchanges/purchases and war recovery. Aid uses the recipient AI's own
verified operating/urgent-defense shortfall, preserves the donor's reserves and
does not fund speculative preparation. Maps are valued for newly revealed area;
shared vision retains alliance and leakage safeguards. Peace recovery considers
own deficits and currently visible enemy military pressure, while preserving
ally obligations and treaty progression. Proposals and acceptance share values;
incoming human offers still require approval. There is no open-borders clause.
Validate with the `diplomatic-aid`, `diplomatic-maps` and `diplomatic-recovery`
release scenarios (one normal turn each). Inspect `diplomacy.negotiation_decisions`
and `diplomacy.recent_negotiation_decisions`, then actual acceptance events.
Repeated unmet decisions compact in lean mode and remain complete in verbose.

### Recorded exception: vassal states (Civ IV-inspired)

The owner authorized vassal states on 2026-11-19. See
[`docs/VASSAL_STATES.md`](docs/VASSAL_STATES.md) for the implemented rules and
scope. Preserve directional agreements separately from symmetric pacts, shared
master-controlled war transitions, deterministic AI evaluation, human approval
of incoming offers and independent city/unit ownership. Do not equate military
access with alliance scoring or transport rights. Mechanical thresholds live in
`content/vassalage.toml`. Validate with the `vassal-capitulation` and
`vassal-voluntary` release scenarios and the source-adjacent rule/save tests.
For autonomous AI-to-AI behavior, run `ai-vassal-capitulation` and
`ai-vassal-voluntary` for two turns, `ai-vassal-war-peace` for five, and
`ai-vassal-independence` for three. These use normal turns with AI nations in
slots 1 and 2; preserve shared acceptance policy for submission and conqueror
requests, and consider capitulation before routine ceasefires.
Colonies and resource tribute are deferred.

### Recorded exception: religion (Civ IV-inspired)

The owner authorized the religion plan and implementation on 2026-11-20.
See [`docs/RELIGION_IMPLEMENTATION_PLAN.md`](docs/RELIGION_IMPLEMENTATION_PLAN.md).
Religion is a custom extension: automatic city influence and trade spread,
founding, declared faith and Tolerant/Devout/Missionary policies, independent of
government and future civics. Keep religious units out of this system.
The owner subsequently chose the seven fixed Civ IV religion names, each
claimed once globally and unlocked by its catalog technology. Meditation
(Mysticism prerequisite, optional Ancient) and Divine Right (Theology plus
Monarchy, optional Medieval) are authorized custom technologies. Preserve
legacy technology IDs and the Future/Unset sentinels; new advances use 103/104.
Parameters and faith bonuses live in `content/religion.toml`. Preserve deterministic snapshot
resolution, visibility-safe notifications, shared human/AI command legality,
and existing treaty/war obligations. Validate with the `religion-spread` (12
turns), `religion-competition` (2 turns), and `religion-ai` (2 turns) release
scenarios plus `religion-tech-unlocks` (2 turns) and source-adjacent tests.

Religion AI also uses the shared economy/science/war/space priorities and its
own culture progress, unrest and known peaceful connections. Founding and
policy scores use actual bonuses; diplomacy considers both public directions
and weights allies. Optional research interest is bounded by cost/path length
and yields to recovery, war, preparation, a space program and strong existing
advisor requests. Religion only breaks ties between equal native trade values;
route legality and replacement remain native. No opponent's private religion
or research queue enters the policy. Validate with `religion-ai-goals` and
`religion-ai-coalition` (one turn each), `religion-ai` (two turns), and the
`shared-strategy` regression, plus the source-adjacent policy/trade tests.
For focused research and spreading regressions, run `religion-ai-research-race`,
`religion-ai-rival-claim`, and `religion-ai-recovery` for one turn each, and
`religion-ai-missionary` for five turns. These record actual selections/events;
the last includes a parallel Tolerant control. Use seeds 42, 7 and 99.
For diplomatic effects, run `religion-diplomacy-shared`,
`religion-diplomacy-different`, `religion-diplomacy-tolerant`, and
`religion-diplomacy-war` for one turn each at those seeds. Each has a parallel
control and checks actual treaty acceptance or normal optional-war preparation;
religious tension must not produce an immediate treaty-bypassing war.
Policy effectiveness additionally uses `religion-ai-culture` (three turns) and
`religion-ai-saturated-spread` (eight turns), with parallel controls and actual
history/influence checks at seeds 42, 7 and 99. Value marginal capped pressure
and native mood counterfactuals; do not reward Missionary for pressure that the
incoming cap discards, or Devout for content already covered by other effects.

### Recorded exception: aircraft (Civ III Conquests)

The owner authorized Civilization III: Conquests aircraft rules for fixed-wing
aircraft (`UnitClass::Air`: Fighter, Bomber, Jet Fighter, F-15, Stealth Fighter,
Stealth Bomber) on 2026-11-21. Fixed-wing aircraft are base-only units stationed
in allied cities, friendly/neutral Airfields/Airstrips without hostile units, or
allied Carriers with capacity. They cannot move tile-by-tile over open terrain,
cannot make direct melee attacks against land/naval units, do not suffer
fuel-exhaustion crashes or field attrition, and do not defend against ground or
naval attacks; enemy fixed-wing aircraft stationed in a captured city or overrun
airfield are destroyed. Each fixed-wing aircraft has a one-mission-per-turn
budget (`moves = 1`). Missions are `Bombard` and `PrecisionBombard` (Stealth
aircraft with Smart Weapons) within `operational_range`, `Recon` (revealing a
3×3 area around any tile within `operational_range`), `Rebase` (relocating
between valid bases within `3 * operational_range`), and `AirSuperiority`
(Fighter-class CAP defending within `operational_range / 2`, once per turn, 50%
normal / 5% stealth detection, air-to-air combat; any interception aborts the
incoming mission even if the incoming aircraft survives). Fixed-wing bombard
runs also face passive anti-aircraft defense: city `SAM_BATTERY` (50% normal /
5% stealth fire chance, `8 / (8 + defense)` kill chance, firing aborts the run
even on miss) and up to 4 strongest hostile tile AA units (`air_defense > 0`),
each rolling `air_defense / (air_defense + 10 * defense)`. Helicopters and
missiles retain their existing rules. AI counter-air production (`SAM_BATTERY`,
`MobileSAM`/`Flak`, and CAP fighters) responds only to observable enemy air
threats (visible aircraft or recent air raids/interceptions), never omniscient
fog peeking. Validate with `air-counter-defense` (6 turns), its parallel
control `air-counter-defense-control` (6 turns), and `air-warfare` (10 turns)
at seeds 42, 7 and 99, plus source-adjacent tests in `src/sim/aviation/tests.rs`.

### Recorded exception: nuclear weapons & fallout (Civ III Conquests)

The owner authorized Civilization III: Conquests nuclear weapon rules on
2026-11-21. A nuclear detonation affects the full 3×3 square (ground zero plus
all 8 Civ III compass neighbors, 9 tiles total) rather than Freeciv's 7-tile
ISO|HEX radius. Every non-water, non-city land tile inside that 3×3 blast zone
deterministically receives nuclear fallout (`tile.fallout = true`) and loses its
terrain improvements (roads, railroads, mines, irrigation, farmland, oil wells,
and bases); water tiles (Coast, Sea, Ocean, Lake) and surviving city-center
tiles do not receive tile fallout. A nuked city loses half its population
(`size / 2`, minimum size 1; nukes never raze a city), caps food to its new
granary, and rolls an independent 50% destruction chance for each regular city
improvement (`Genus::Improvement`), preserving the Palace, Small Wonders, and
Great Wonders. Every unit caught in the 3×3 blast zone rolls an independent 50%
destruction chance; survivors lose half their remaining HP (`(hp + 1) / 2`,
floored at 1 HP). `SDI_DEFENSE` is an empire-wide Small Wonder that intercepts
enemy nuclear strikes targeting any city, tile, or unit of its owner inside the
3×3 blast zone with a 75% chance against ICBMs and 37.5% (75% detection with
50% tactical evasion) against Tactical Nukes. Validate with `nuclear-warfare`
(3 turns at seeds 42, 7 and 99) and the tests in `src/sim/nuclear.rs`.

## Comparison source checkout

The Freeciv source submodule at `reference/freeciv` tracks the public upstream
`main` branch. It is the gameplay authority for mechanics, but the pinned revision
recorded in the port plan governs what CivCraft ports. Consult it for any
gameplay logic, formula, or state transition. Content records (units, buildings,
civilizations) still come from Civ III, and Freeciv's content does not replace
them. For example, the Civ2Civ3 unit rules are in
`reference/freeciv/data/civ2civ3/units.ruleset`; the Civ III unit catalog
overrides its unit identities and values.

## Choosing the right test path

Use the narrowest test path that exercises the behavior being changed:

- Simulation rules, turn processing, AI behavior, or deterministic gameplay
  scenarios: use the headless simulator. Prefer a registered scenario for a
  reusable regression and `--load-campaign` for an elaborate saved starting
  state. See `docs/HEADLESS_ANALYTICS.md`.
- Headless executable arguments, exit status, and report contracts: add an
  `assert_cmd` integration test under `tests/`. Do not duplicate engine-rule
  assertions at the CLI layer.
- Rendering, layout, screenshots, or visual regressions: use `--ui-review`.
  Compare against an explicit baseline for regression work; never update a
  baseline implicitly. Run it only when `CIVCRAFT_SOURCE_DIR` is set and points
  to a complete Civ III installation. Otherwise skip visual testing, report the
  missing prerequisite, and do not describe the skipped review as passing. See
  `docs/UI_REVIEW.md`.

Run the regular Rust suite locally with Cargo:

```sh
cargo test --all-targets
cargo test --doc
```

Use focused test filters while iterating. Run the broader commands before
hand-off when the change warrants it. Keep the separate `cargo test --doc` step
because `--all-targets` does not run documentation tests. Nextest is for CI:
CI runs `cargo nextest run --profile ci --all-targets` using
`.config/nextest.toml`, followed by `cargo test --doc`.

Run headless simulations in release mode: use
`cargo run --offline --release --bin headless_sim -- ...` or a freshly built
`target/release/headless_sim`. This applies to simulation analysis, scenario
runs, and profiling. Use a debug build only when debug-specific diagnostics are
needed; explain that choice before starting the run. The Cargo test commands
above remain unchanged. Choose the shortest scenario or turn count that reaches
the behavior under review. Use focused fixtures for late-game transitions;
reserve 200-turn runs for questions that need long-term simulation evidence.

For analytics review, start with `report.json` findings and the named summaries
under `analytics.metrics` (`government.nations`, `government.recent_changes`,
`diplomacy.relationships`, and `diplomacy.recent_activity`). Follow their evidence
references into the sidecars instead of dumping full JSONL streams. Default
`--report-detail lean` output includes important adapter changes, compact events,
and full snapshots at checkpoints and finding boundaries. Read
`evidence.changes` for the change stream. Use `--report-detail verbose` for full
per-boundary snapshots and every selected event. Analyzers receive all collected
events and full snapshots in either mode. Verbose evidence uses lossless deltas
and shared values. Retrieve full snapshots with
`cargo run --offline --bin report_read -- report.json --turn N`;
use `--stream events --sequence N` to retrieve an event. Do not parse encoded
verbose sidecars as standalone snapshots/events; keep the report and all
sidecars together.

Use `jq` for entity, domain and turn-range queries, projections, grouping and
comparisons. Decode verbose evidence with `report_read` first; query lean change
streams directly. Prefer reusable `jq` recipes over adding a custom query layer
unless a demonstrated need cannot be handled clearly with these tools. Match
diplomatic events against any participant, not just the initiating player.
For example, inspect government/diplomacy events involving civilization 2:

```sh
cargo run --offline --bin report_read -- report.json --stream events |
  jq -c --argjson nation 2 --argjson start 90 --argjson end 160 '
    select(.turn >= $start and .turn <= $end)
    | select(.source == "government" or .source == "diplomacy")
    | select(any(.entities[]?; .kind == "civilization" and .id == $nation))
    | {turn, phase, sequence, kind, payload}'

jq -c --arg owner "2" '
  .turn as $turn | .changes[]
  | select(.provider == "governments" and .key == $owner)
  | {turn: $turn, before, after}' report.json.changes.jsonl
```

Start with lean mode for general reviews. Choose verbose mode when the question
requires exact per-turn food, research progress, treasury, movement, work
countdowns, or every government decision; when a missing intermediate snapshot
blocks a diagnosis; or when validating an adapter or compaction rule. Rerun the
same seed/world settings or saved campaign with a focused turn range and relevant
providers/domains. Check coverage and retention metadata before treating an
absent record as an absent action: verbose mode still honors trace filters and
limits, and cannot expose uninstrumented actions. Enable `--profile-turns true`
separately when investigating execution cost. See `docs/HEADLESS_ANALYTICS.md`
for coverage and retention semantics.

### Headless scenarios and saved campaigns

Registered gameplay definitions and their catalog belong in `src/scenarios`.
Use `sim/fixtures` for typed state preparation, and keep private rule assertions
in ordinary source-adjacent `tests` modules under `cfg(test)`. Shared simulation
checks enter through `sim::tests::gameplay`; subsystem helpers belong under their
local `tests` module. Keep root `tests/` for executable/CLI contracts. The headless
runner owns all full
turn advancement, including preparatory turns requested by scripted actions.
Scenarios reach state-mutating rule steps only through the `sim::drive::Drive`
trait (import it explicitly); keep those `Simulation` methods `pub(in crate::sim)`
and add a wrapper there rather than widening them to `pub(crate)`. Read-only
queries may stay `pub(crate)`.
See `docs/SCENARIOS.md` for the scenario architecture.


List and run registered scenarios without creating a graphics context:

```sh
cargo run --offline --release --bin headless_sim -- --list-scenarios
cargo run --offline --release --bin headless_sim -- \
  --scenario campaign-readiness-presets --seed 42 --turns 1 \
  --report /tmp/civcraft-readiness.json
cargo run --offline --release --bin headless_sim -- \
  --seed 42 --turns 200 --save-campaign /tmp/civcraft-seed42-t200.save \
  --report /tmp/civcraft-seed42-t200.json
cargo run --offline --release --bin headless_sim -- \
  --load-campaign /tmp/civcraft-seed42-t200.save --turns 50 \
  --profile-turns true --report /tmp/civcraft-late-check.json
```

For late-game performance profiling or AI unit/economy iteration, create a
campaign checkpoint once (unprofiled `--release`, up to the era of interest)
with `--save-campaign` and replay focused 20–50 turn windows with
`--load-campaign` instead of re-running from turn 1. Use an earlier checkpoint
(e.g. turn 200) when testing AI planning changes whose effects accumulate before
the endgame, or a later checkpoint (e.g. turn 350+) for pure endgame profiling.
Regenerate the checkpoint if the save schema changes, and run a full campaign
pass only for final verification.

Require a zero exit status and inspect `summary.scenario_checks_passed`,
`summary.scenario_checks_total`, and every entry under `run.scenario.checks`.
Failed scenario checks make the command fail after it publishes the report.
Use `ScenarioKind::Runtime` with an explicit version for scenarios that exercise
normal campaign turns. Choose control and runtime-world provenance explicitly.
Build the fixture with `WorldBuilder` before the initial snapshot, track the
exact entities by ID, and let the headless runner advance the game. Observe
state after each turn and evaluate required outcomes in `finish`, including
when the run ends early. Inspect `run.scenario.observations` for diagnostics.
Scripted command regressions can use scenario actions after tracing is attached.
Keep reusable setup in simulation-level helpers. See
[`docs/HEADLESS_ANALYTICS.md`](docs/HEADLESS_ANALYTICS.md#scenario-inputs) for
the builder and registration examples.

For focused screenshot galleries and visual review, see `docs/UI_REVIEW.md` and
the `--ui-review` instructions in `README.md`. For a visual regression
comparison, supply both the current output and an explicit baseline:

```sh
./target/release/civcraft --seed 42 \
  --ui-review /tmp/civcraft-ui-current \
  --ui-baseline /path/to/civcraft-ui-baseline \
  --ui-min-similarity 0.99 --window-size 1280x720
```

Treat a missing baseline, dimension mismatch, similarity below the threshold,
or any failed report check as a failure. Review generated `*-diff.png` files
and the `baseline_similarity` values in `report.json`. Do not copy current
captures over the baseline unless the user explicitly approves a baseline
change.

For this checkout, the Civ III art source is available at
`$HOME/Documents/projects/CivCraft/source`. Set it before launching any game
window or screenshot/visual-review run:

```sh
export CIVCRAFT_SOURCE_DIR="$HOME/Documents/projects/CivCraft/source"
```

Before launching `--ui-review`, verify that the configured installation has
the required terrain assets:

```sh
test -n "${CIVCRAFT_SOURCE_DIR:-}" \
  && test -d "$CIVCRAFT_SOURCE_DIR/Conquests/Art/Terrain"
```

If that preflight fails, do not run `--ui-review` or a baseline comparison in
that environment. Continue with applicable headless and Rust tests and state
that visual testing was skipped because the Civ III installation was not
available. Do not substitute fallback, generated, or partial assets for an
authoritative visual baseline.

## Checking map-generation goldens

Campaigns use Civ III's horizontal map topology and 36×48 tile preset. All
campaign maps wrap horizontally and have bounded polar edges. Generate maps
with the desired dimensions from the start; changing `sim.map_wrap` after
generation leaves arrays inconsistent.

`tests/fixtures/freeciv_mapgen_vectors.tsv` contains raw Freeciv 3.3.90.14-dev
(eb8c703) terrain, altitude, extras and starting locations for historical
comparison only. It is not the Civ III target. The parity test checks the
unmodified legacy generator against this file.
`tests/fixtures/civcraft_mapgen_vectors.tsv` contains checked CivCraft output
using the Civ III resource catalog. Resource placement changes extras and start
site scoring, so keep these goldens separate from the native reference.

The historical generator is no longer distributed with this repository. If a
fixture must be replaced, reproduce it independently with the exact recorded
Freeciv revision and reference Civ2Civ3 data. The reference server binds
localhost even without clients, so sandboxed runs may need desktop/network
approval. Unit goldens do not validate rendering.
