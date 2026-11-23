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
calling the observer. An ended comparison stays stopped. Observers collect evidence
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
`trait-campaign`. The remaining sixteen `Actions` definitions control individual
rule inputs or transitions. They retain `Drive` where they need a rule result or
an explicit phase boundary, rather than scheduling an AI player's activities.

| Definitions | Controlled contract |
| --- | --- |
| `combat-matrix` | Movement-triggered combat results under terrain and city-defense variants. |
| `city-healing` | Unit restoration with and without the relevant healing improvement. |
| `culture-history`, `lifecycle-history`, `research-history` | History/provenance for explicit production, capture, loss, research, treaty and upkeep operations. |
| `culture-borders`, `culture-wonder-capture`, `culture-empire-victory` | Culture milestones claim territory; wonder sources age and follow their original builder through capture and recapture; distributed empire culture wins at the normal turn boundary. |
| `culture-border-growth` | Nine normal turns of culture-only borders: a lone capital grows 9, 21, 45, 77 and 117 tiles at the 10/100/1,000/10,000 thresholds (staged culture, one Palace point from each), while two borders meet under the Civ III contest: the nearer city owns a tile, and culture decides only the equidistant one. Run with `--turns 9`; seeds 42, 7 and 99. |
| `border-contests` | Six normal turns of border contests, each a separate case: the nearer city owns a tile whatever the sizes (size 12 against size 1); culture decides an equidistant tile and flips it as culture is staged (turns 3 and 5); equal culture goes to the older city (human-founded and AI-founded); three civilizations meet at one junction; two cities two steps apart keep their own tiles. Every turn it re-derives the whole map's ownership from the rule and compares. Run with `--turns 6`; seeds 42, 7 and 99. |
| `border-land-claims` | Two turns of what a border may claim: a coastal city holds land and sea, an island city's fat X is all ocean, tiles the owner has not seen are claimed and revealed, a border wraps across the map seam, and a polar border is clipped. Run with `--turns 2`. |
| `border-city-loss` | Seven turns: on turn 3 one city of a shared-border pair is destroyed (its tiles are released and the other city inherits the middle tile) and another is captured (it falls back to its nine-tile square under fresh culture, and its tiles change owner). Run with `--turns 7`. |
| `great-leader` | Leader consumption and completion of a selected city improvement. |
| `expansion-balance`, `shared-strategy`, `wartime-funding` | Advisor/cache repricing, shared policy weights, purchase quotes and upkeep/reserve constraints under changed inputs. |
| `proactive-trade`, `peaceful-first-contact`, `relationship-memory`, `competing-trade-partners` | Atomic diplomatic choices, valuation, first-contact rules and directional relationship memory. |

`government-situation-war`, `-peace` and `-cycle` are matched runtimes: militaristic
Rome and otherwise identical India run in a primary and a comparison campaign.
War checks that Rome holds a war-family government, peace that India reaches the
peace family while Rome keeps its military government, and the cycle that war,
peace and war again produce bounded revolutions with no loops or flip-flops.
`government-situation-disorder` starts a Republic whose city has been rioting and
checks that it leaves once and cannot return before the next re-evaluation. Run
with `--turns 8`, `8`, `24` and `4`. `government-situation-locked` (`--turns 6`)
removes The Republic at peace and checks that no unavailable government is
adopted. Use seeds 42, 7 and 99. A source-adjacent test runs all five under
`cargo test`; the unit tests cover missing civilization records, barbarian
wars, no available alternative, all-zero scores, nations without cities and
the cached instability block expiring.

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
| `space-industrial-loss` | 83 | 150 | Removes the highest-output eligible non-capital city building a spaceship part once at least five of the ten parts exist. Checks a surviving city takes on the interrupted part and the space-race victory with the frontier guarded during war. |
| `naval-second-wave` | 42 | 50 | After the first capture, adds reserves on the home shore and an enemy counterattack on the far shore. Tracks the original reserves across the water and checks whether the port holds. |
| `competing-trade-partners` | 42 | 1 | Builds cooperation and grievance through gameplay, equalizes current relations, then submits the same military bid from both partners and a low-risk bid from the partner who broke the pact. |

The runtime scenarios apply fixture changes before the next full turn. In a
paired run the runner advances both campaigns. Read the comparison under
`run.scenario.observations` and individual results under `run.scenario.checks`.

## Desktop release feature footprint

Measured at commit `227332b84e946f8bc58fd9159c9d8413b530c51c` on Linux x86_64
(glibc 2.41), using `rustc 1.100.0 (0ef1bc386 2026-11-09)`. Both builds use the
repository's release profile: optimization level 3, ThinLTO and one codegen unit.
The only feature difference is `scenarios`.

| Measurement | `desktop-audio` | `desktop-audio,scenarios` | Difference |
| --- | ---: | ---: | ---: |
| Unstripped ELF bytes | 10,370,104 | 10,363,696 | -6,408 |
| Stripped ELF bytes | 9,012,800 | 9,005,952 | -6,848 |
| `.text` bytes | 6,717,800 | 6,710,968 | -6,832 |
| `.rodata` bytes | 1,203,982 | 1,203,982 | 0 |
| Defined scenario symbols | 0 | 0 | 0 |
| Defined `Drive` symbols | 0 | 0 | 0 |

The scenario-enabled binary is 0.062% smaller before stripping and 0.076% smaller
after stripping. Neither binary retains symbols under `civcraft::scenarios::` or
`civcraft::sim::drive::`. This measured difference does not justify changing the
default features. Keep the desktop-only CI build to verify the feature boundary;
these size results describe this revision, platform and toolchain.

To reproduce with GNU binutils installed, build both variants from the same clean
revision and preserve each executable before building the next:

```sh
cargo build --locked --offline --release --no-default-features \
  --features desktop-audio --bin civcraft
cp target/release/civcraft /tmp/civcraft-desktop-only
cargo build --locked --offline --release --no-default-features \
  --features desktop-audio,scenarios --bin civcraft
cp target/release/civcraft /tmp/civcraft-with-scenarios

for binary in /tmp/civcraft-desktop-only /tmp/civcraft-with-scenarios; do
  cp "$binary" "$binary.stripped"
  strip --strip-unneeded "$binary.stripped"
  wc -c "$binary" "$binary.stripped"
  size -A "$binary"
  nm --defined-only --demangle "$binary" \
    | rg 'civcraft::(scenarios|sim::drive)::' || true
done
```

### Vassal states

`vassal-capitulation` and `vassal-voluntary` prepare a threatened AI, its
prospective protector and an independent third nation through `WorldBuilder`.
Traced actions run the existing AI proposal and player acceptance command
paths; a normal turn then checks that AI planning and treaty expiry preserve
the agreement. Run each for one turn.


The all-AI vassal runtimes start with independent nations in slots 1 and 2.
They form agreements entirely through ordinary turns; slot 0 is a separate AI.
Run `ai-vassal-capitulation` and `ai-vassal-voluntary` for two turns each,
`ai-vassal-war-peace` for five, and `ai-vassal-independence` for three.
The latter two inject a prepared war plan or the loss of two remote cities
through `before_turn`, then observe the AI's response. They never invoke
submission, acceptance, war declaration, peace negotiation or independence rule
steps directly. Inspect their scenario observations and `diplomacy.transition`
events for the matching AI participants, actual acceptance, group pact changes
and `protection_failed` departure. These controlled island fixtures establish
behavior under explicit conditions, rather than campaign-wide frequency or balance.

### Religion

The custom religion extension has sixteen normal-turn scenarios:

| Scenario | Turns | Required outcome |
| --- | --- | --- |
| `religion-ai-culture` | 3 | A culture-focused AI chooses Tolerant and gains more actual history than a parallel Devout control |
| `religion-ai-saturated-spread` | 8 | Three own sources already cap incoming pressure; Tolerant converts five targets at the same rate as a Missionary control while producing more culture |
| `religion-diplomacy-shared` | 1 | Shared declared faith tips a marginal alliance offer into acceptance; the undeclared control refuses |
| `religion-diplomacy-different` | 1 | Different Devout faiths block a marginal alliance that the undeclared control accepts |
| `religion-diplomacy-tolerant` | 1 | Tolerant removes the receiver's different-faith penalty and permits the same alliance; the other direction stays hostile |
| `religion-diplomacy-war` | 1 | Religious tension tips a marginal optional war plan into preparation while both campaigns remain at Peace |
| `religion-ai-research-race` | 1 | An AI-selected religious research project completes and the AI founds its unlocked religion |
| `religion-ai-rival-claim` | 1 | A rival claim removes the old religious incentive; the AI selects and founds an alternative |
| `religion-ai-recovery` | 1 | A financial shock removes optional religious research interest while normal accounting continues |
| `religion-ai-missionary` | 5 | AI chooses Missionary and converts two tracked cities before a parallel Tolerant control |
| `religion-ai-goals` | 1 | Science, culture and economic priorities lead three AI nations to different appropriate religions |
| `religion-ai-coalition` | 1 | An AI adopts an ally's present minority faith while preserving the alliance |
| `religion-tech-unlocks` | 2 | Normal research completes Meditation and Divine Right; Buddhism and Islam can then be founded |
| `religion-spread` | 12 | A prepared domestic minority becomes dominant and a foreign trading partner gains presence |
| `religion-competition` | 2 | Competing minorities stay mixed; different declared faiths produce directional hostility while Peace remains intact |
| `religion-ai` | 2 | An AI founds a faith and chooses a beneficial declaration/policy through normal turn processing |

`religion-spread` starts its domestic target with 90 influence to keep the
regression short; it does not start converted. The competition target has two
180-influence minorities. Founding/declaration use `Drive`; influence preparation
uses `WorldBuilder::city_religion`. Observations include raw city influence,
dominance and national faith/policy state. Run with `--snapshot-providers
cities,civilizations,religion,diplomacy` to inspect
transitions. The source-adjacent tests cover spread from an initially empty
city and save/resume behavior as well.

The research-race fixtures start with the other technologies known and a stable,
content government so unrelated discoveries/revolutions do not obscure the
policy test. They call the ordinary AI research selector, record its legal
choice and interest, and only then prepare the final research beaker. Research
completion and founding must occur through a normal turn, with matching trace
events. These are focused end-of-project regressions, not benchmarks of a full
research campaign from turn one.

The rival-claim variant founds the selected religion for a rival before the AI
reevaluates. The recovery variant instead installs an explicit maintenance
shock and checks that the religious incentive falls to zero. It does not assert
that all research stops: native research may remain useful for other reasons.

The Missionary fixture starts two domestic minorities at 90 influence. Its
comparison game has the same AI control and starting state but holds Tolerant
policy through a valid declaration cooldown. The runner advances both games
once per turn. Five turns must produce two conversion events in the AI-selected
Missionary game while both control cities remain mixed. All four new scenarios
are exercised at seeds 42, 7 and 99 in source-adjacent tests.

The four diplomacy counterfactuals clone the same starting world and change only
the AI's declared faith or policy. Both legal faith choices are already present
in its capital; valid declaration cooldowns preserve the comparison during the
normal turn. Alliance cases submit the same affordable 5,000-gold offer through
the ordinary human treaty command, exercising actual AI acceptance and atomic
execution. Initial raw trust is 980 for the shared-faith case and 1,020 for the
different-faith cases, placing the receiver near the alliance love threshold of
100. Observations retain initial and final directional trust/faith attitudes,
pacts, accepted offers, war plans and strategy. The war case starts at -2,540 raw
trust; native peaceful attitude gain/decay takes both runs to -2,290 before
planning. Neutral faith does not cross the strict optional-war threshold, while
Devout's -4 attitude shifts it by 40 trust units. It requires identical final
raw trust, a `war_plan_started` event, and checks that preparation has
not bypassed the existing Peace treaty. These are controlled decision boundaries,
not evidence that religious differences usually cause war or prevent alliances.

The effectiveness scenarios compare actual history accumulation and city
influence through normal turns. Both start three Hindu-majority cities; the
capped-spread case also prepares five nearby minorities at 90 influence. Each
target already receives the six-point incoming cap without Missionary policy.
The control holds its legal Devout/Missionary policy for the ten-turn cooldown.
The AI must stay Tolerant, earn at least three additional history points per
turn, and, in the spread case, convert all five cities with exactly the same
influence as the Missionary control. Run both at seeds 42, 7 and 99.
