# Religion implementation plan

Status: first implementation complete on `main`; balance values remain starting
values for playtesting. The implementation includes simulation, persistence,
AI, management/city/diplomacy UI, notifications, a religion observation provider,
and the normal-turn scenarios listed in `SCENARIOS.md`.

Access Religion from the government screen. Research the required technology, choose an unclaimed predefined religion
and an owned founding city, and found it. Then
preview and confirm a declared faith and policy. City summaries show influence
shares; the screen explains conversion thresholds. Government and future civics
remain independent. The `religion` UI review scene exercises declaration and
policy confirmation; visual comparison still requires the documented assets and
an explicit baseline.

## Scope and authority

The owner requested a simple Civ IV-inspired religion system: city influence,
automatic spread without religious units, a declared civilization faith,
customizable effects, diplomatic tension, and conversion notifications.
This is an intentional custom gameplay extension to the Freeciv baseline.
Existing government, treaty legality, military obligations, city ownership,
and victory rules continue to govern their respective systems.

The first release includes founding, city influence, automatic spread, declared
faith, one policy choice, AI participation, UI, persistence and diagnostics.
Defer religious units, a separate faith currency, religious victory, persecution,
religious buildings, belief trees and holy-war treaty clauses. Civics will be a
separate feature; religion must work under every government.

All numbers below are proposed starting values, owned by ruleset configuration
and subject to scenario tuning. They are not Civ IV parity claims.

## Player decisions

A civilization may found one religion, and each predefined religion can be
claimed once worldwide. Researching its required technology unlocks the option;
founding is a deliberate command shared by humans and AI. Players choose an
owned city and a catalog entry; free names and emblems are not accepted.

| Religion | Required technology | Bonus where dominant |
| --- | --- | --- |
| Buddhism | Meditation | +1 content citizen |
| Hinduism | Polytheism | +1 culture per turn |
| Judaism | Monotheism | +1 gold per turn |
| Confucianism | Code of Laws | +1 science per turn |
| Taoism | Philosophy | +1 local religious influence per turn |
| Christianity | Theology | +1 outgoing spread pressure per eligible edge |
| Islam | Divine Right | +2 outgoing trade-route pressure |

These are CivCraft gameplay bonuses, not historical/religious claims or Civ IV
parity. Benefits follow the dominant faith in each city, regardless of its
owner's declared faith. They stack with the separate national policy. Gold and
science are flat additions after building output multipliers and remain subject
to the normal city processing/disorder rules. Spread still obeys the shared cap.

Meditation has base cost 6, requires Mysticism, and is an optional Ancient branch beside
Polytheism. Divine Right has base cost 48, requires Theology and Monarchy, and is an
optional Medieval branch beside Education and Printing Press. Neither is
required for advancing eras. Their appended persistent IDs are 103 and 104;
Future Tech (101), Unset (102), and all existing technology IDs remain unchanged.
The tree reuses existing Mysticism/Theology artwork for these custom advances.

The founding city gains 300 influence and remains its historical holy city.
Founding does not automatically declare a faith; the UI offers that separately.

A civilization can declare any faith present at 20% or more in one owned city,
or choose no declared faith. Its cities retain their own beliefs when this
choice changes. A declared faith is a diplomatic position, not a government.

Choose one policy for the civilization, independently of its faith:

| Policy | City/spread effect | Diplomatic attitude toward a different declared faith |
| --- | --- | --- |
| Tolerant (default) | +1 culture per turn in cities with any dominant faith | 0 |
| Devout | Convert one unhappy citizen to content in cities dominated by the declared faith; +2 local influence per turn for that faith in those cities | -4 |
| Missionary | +2 outgoing pressure along each eligible edge carrying the declared faith | -2 |

Matching declared faith gives +4 attitude under every policy. No declared faith
on either side gives no religious attitude modifier. Devout and Missionary have
no mechanical benefit without a declared faith. Show these consequences before
confirming a change. The happiness effect enters the existing city happiness
calculation without bypassing its ordering, specialist rules or unrest checks.

Faith and policy can be changed together through one command, with a shared
10-turn cooldown after a successful change. No anarchy or treasury charge.
Founding does not consume this cooldown. Changing to no declared faith retains
the selected policy. Invalid and unchanged commands consume nothing.

## City influence and conversion

Store influence separately from culture. Culture continues to control its
existing growth, border and victory behavior; religious influence measures the
relative strength of competing faiths inside one city.

Each city has a sparse map of religion ID to integer influence, bounded to
0..1000 per religion. Add a constant unaligned weight of 100 when calculating
shares. Display each faith's percentage and an unaligned remainder; use a
stable rounding rule so the displayed shares sum to 100%. Use exact integer
comparisons for gameplay thresholds, not rounded UI percentages.

A faith is present at a share of at least 20%. A city gains a dominant faith
when a candidate has at least 50% share and leads every other faith by at least
10 percentage points. An incumbent remains dominant while its share is at
least 40%, unless a challenger meets both takeover requirements. Otherwise the
city becomes mixed with no dominant faith. Ties never choose an arbitrary winner.
These separate thresholds prevent repeated conversion messages near a boundary.
Dominance controls policy benefits; minority faiths remain and may later recover.

Resolve religion once per completed turn, after city/world changes and before
`advance_turn_clock` advances diplomacy. Snapshot surviving cities and eligible
trade routes first. Calculate every city's result from that snapshot, then apply
all results together. A conversion cannot start a chain across the map in the
same turn. New policy benefits affect the next city processing phase.

For every existing influence entry, subtract ceil(influence / 100), then add:

- +4 for its faith in the historical holy city, regardless of city owner.
- +2 for the incumbent dominant faith, representing an established community.
- Applicable local policy pressure.
- Pressure from eligible neighboring cities and established trade routes.

A source exports only its dominant faith: +2 to cities of the same owner within
map distance 4, or +3 through an existing active trade route. A city pair uses
its strongest eligible edge, rather than adding both routes. Foreign spread
requires a legal active trade route and peaceful relations; there is no invented
open-borders requirement. Deduplicate reciprocal trade records by city pair.
Missionary adds its outgoing bonus when source dominance matches its owner's
declared faith. Cap incoming external pressure at 6 per faith per target city
per turn, then clamp total stored influence to 1000. Prune zero entries.

For example, a city receiving +3 each turn from one trading partner accumulates
influence gradually. An unopposed faith becomes present around 25 influence and
can dominate at 100; an established competing community takes longer to displace.
These timings must be checked with normal-turn scenarios before release.

Track the largest contributing source for each faith during a turn, breaking
equal contributions by city ID. Conversion attribution uses that source's
current owner, with local/holy-city growth labeled local. Describe contribution,
not ownership of a foreign city's beliefs. Do not persist per-turn source edges.

Conquest retains influence and dominance. Razing deletes city influence; the
religion retains its name and founding history but loses its holy-city pressure
if that city no longer exists. New cities begin unaligned. Founders can be
eliminated without deleting their religions. No automatic founding or conversion
is applied to old saves.

## Diplomacy and war

Add a bounded, directional religion attitude component alongside existing
relationship memory. Calculate it from current declared faiths and the evaluating
civilization's policy; do not repeatedly record it as a cooperation/grievance event.
Expose the religious component separately in diplomacy explanations and tracing.

Use that attitude through the existing acceptance and optional war evaluation
paths, auditing those paths to avoid applying the component twice. Religious
differences can help tip a hostile, prepared AI toward an existing legal war
choice. They never issue a war command directly. Preserve pact cancellation,
Senate restrictions, vassal/master transitions, ally obligations, economic
recovery and military readiness checks. Shared faith cannot guarantee peace.

Do not add a separate religious tension meter in the first release. If a war
has religious hostility among its actual decision inputs, include that in its
explanation without claiming religion was its sole cause. Do not infer religion
from a civilization's name, traits or real-world history.

## Notifications and presentation

Reuse `src/sim/notifications.rs` and `src/client/hud/notifications.rs`.
Append a Religion category, preserving serialized enum discriminants.
Use contextual city/civilization references and existing unread/read behavior.

| Event visible to the human | Delivery |
| --- | --- |
| You found a religion or change your declared faith/policy | Important: toast and inbox |
| One of your cities changes dominant faith, including becoming mixed | Important: toast and inbox |
| A faith first reaches 20% in one of your cities | Routine: inbox |
| Your city's pressure helps a foreign city first gain presence or convert | Routine: inbox, only if destination information is observable |
| A contacted civilization publicly changes declared faith | Routine: inbox |

Emit on state transitions, once per event and recipient. Do not send messages
for every influence increment or repeat first-presence notifications after a
faith falls below and recrosses 20%; persist a small per-city announced-faith set.
A conversion may be announced again after a genuine later change. If first
presence and conversion coincide, send only the conversion message. Never reveal
an unknown city or a hidden source through text, attribution or context links.
Examples: “Memphis has adopted the Dawn Faith” and “The Dawn Faith has reached
Athens through trade with Memphis.”

Add a Religion section to the existing civilization management screen, with
catalog founding/faith/policy controls, cooldown and effect previews. Add compact city
faith/share details and a progress explanation for incoming pressure. Show a
contacted civilization's public declared faith in diplomacy. Adapt wording and
layout to the existing UI; do not require new external art or a new HUD button.

## Implementation sequence

1. **Rules and durable state.** Add validated `content/religion.toml`, its loader,
   typed religion IDs, fixed catalog definitions, city influence/dominance, founding history,
   civilization faith/policy/cooldown and presence-announcement history. Record
   this owner-authorized extension in AGENTS.md when implementation begins.
   Own state and algorithms in `src/sim/religion.rs` with focused submodules as
   needed. Keep serialized policy IDs stable and state independent of government.
2. **Commands and city lifecycle.** Add shared founding/declaration/policy commands
   via `src/sim/commands.rs` and `src/sim/drive.rs`, client actions and typed
   queries. Wire new-city, capture, transfer and raze behavior. Validate ownership,
   technology, IDs, eligibility and cooldown atomically.
3. **Normal-turn simulation.** Implement snapshot-based influence/spread in
   `src/sim/turn.rs`, active trade-route filtering and existing wrap/distance
   helpers. Index nearby candidates and trade adjacency; avoid a map-wide city
   pair scan. Wire culture/happiness benefits through their existing calculation
   paths, preserving all existing building effects.
4. **Diplomacy and AI.** Integrate the explained attitude component with existing
   diplomacy policy. AI founds in its capital, or lowest-ID owned city if needed,
   when eligible; declares the most prevalent owned-city faith when beneficial.
   Evaluate all faith/policy choices with the same city effects, city-count faith
   prevalence and visible diplomatic consequences as humans. Use stable tie
   breaking, the same cooldown, and a configurable minimum improvement to prevent
   oscillation. No privileged knowledge or mandatory faith-based war targets.
5. **Persistence and observation.** Introduce the next unused save format after
   checking main at implementation time; do not reuse 0x2F/0x30. Preserve all old
   readers, vassalage and notification state. Older saves initialize empty
   religions/influence and Tolerant/no-faith policies. Validate IDs, ownership
   references, sizes, bounds and cooldowns. Export religion snapshots and actual
   transition/decision events through existing trace/headless conventions.
6. **UI and notifications.** Deliver management controls, city and diplomacy
   summaries, contextual notifications and effect explanations. Keep simulation
   legality authoritative and hide unavailable information in query views.
7. **Regression scenarios and documentation.** Add normal-turn scenarios for
   founding/spread, competing faiths, declaration/diplomacy and AI participation.
   Update scenario catalog, state ownership and ruleset documentation.

Each step should land on `main` in a coherent, buildable state. Do not expose
unfinished controls before their simulation and persistence paths are complete.

## Validation and release criteria

- Rule tests cover invalid configuration, founding/command legality, influence
  rounding, caps, thresholds/hysteresis and source attribution.
- Spread tests cover trade deduplication, foreign hostility, wrapping, same-turn
  chain prevention, deterministic city iteration and loss of source cities.
- City tests prove policy effects apply exactly once, minority beliefs survive
  capture, and existing building/government output remains consistent.
- Diplomacy tests cover directional policy, no-faith neutrality, matching faith,
  cooldown and preservation of war legality, readiness and vassal transitions.
- Notification tests cover priority, recipient visibility, first-presence history,
  conversion reversal, context, unread state and no replay after save/load.
- Save tests cover new round trips, every supported legacy format, malformed
  religion references and uninterrupted versus resumed normal-turn outcomes.
- Normal-turn scenarios demonstrate human and AI founding, gradual domestic and
  trade spread, stable competing religions, policy effects and religious attitude
  influencing evaluation without causing an automatic war.
- Run focused tests while iterating, then required all-target/doc checks and the
  shortest registered release scenarios that establish each outcome. UI review
  requires the documented Civ III asset preflight and explicit visual baseline.

The first release is complete when human and AI civilizations can participate
under the same rules, conversion is understandable through UI messages, saves
resume deterministically, and religion remains independent of government.

## Validation before the fixed-catalog update

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --offline --all-targets -- -D warnings`: passed (the existing
  vendored miniquad static-reference warning remains outside project linting).
- `cargo test --all-targets`: 1,370 library tests, 2 utility tests and 15 CLI
  tests passed; 1 library test ignored.
- `cargo test --doc`: passed (no documentation tests currently registered).
- `cargo check --offline --no-default-features --features scenarios`: passed.
- Fresh release `headless_sim` runs: `religion-spread` (12 turns),
  `religion-competition` (2 turns), `religion-ai` (2 turns), seed 42: each passed
  its required scenario check, with no collector event drops.
- The Religion UI command flow is covered by a unit test and a staged gallery
  interaction. Screenshot/baseline review was not run: the complete Civ III
  asset installation required by AGENTS.md was unavailable in this environment.

## Fixed-catalog validation

The update adds catalog-unlock/global-claim tests, exact city-output and spread
bonus tests, technology prerequisite/ID/optional-branch tests, and the
`religion-tech-unlocks` normal-turn scenario. Research cards list the religion
founding opportunity; the Religion picker shows requirements, bonuses and
Available/Locked/Already founded status.

The fixed-catalog update passes all-target tests (1,375 library, 2 utility and
15 CLI tests; 1 ignored), Clippy with warnings denied, documentation tests,
formatting, and the build without desktop features. Screenshot review remains
unavailable without the required Civ III assets. Religion bonuses are configured
in `content/religion.toml`; changing them requires rebuilding.

The final release build also passes `religion-tech-unlocks` (2 turns),
`religion-spread` (12 turns), `religion-competition` (2 turns), and `religion-ai`
(2 turns), seed 42. Each reports 1/1 required checks passing and zero collector
drops. The unlock scenario completes Meditation and Divine Right through normal
research processing before founding Buddhism and Islam on the following turn.

## AI strategic use

The AI uses religion as part of its existing strategic policy:

- Founding compares bonuses against shared economy/science priorities, its own
  unhappiness, and culture progress. A civilization approaching a culture goal
  values culture more. Identity names never select a religion.
- Declarations and policies consider own-city benefits, useful domestic spread
  and known peaceful trade connections. Public attitudes in both directions
  matter; allies and established friends have more weight than neutral contacts.
- Optional founding research adds a bounded interest to the ordinary advisor.
  It considers missing prerequisites and research cost, stops after a religion
  is claimed, and pauses during war, preparation, economic recovery, space
  commitment or a strong existing advisor request. Native legality, switching
  penalties and urgent research remain authoritative.
- Missionary AI prefers useful known partners when legal trade opportunities
  have equal native economic value. The trade adapter preserves its original
  economic-value sequence, capacity and replacement rules. Existing profitable
  links are not canceled just to promote religion.
- Other civilizations' private research queues and city beliefs are excluded.
  Foreign spread forecasts use known city sites and public diplomatic ties;
  unseen ownership changes cannot reveal the new owner's identity.

The `[ai]` table in `content/religion.toml` owns these weights and search limits.
`religion.ai_decision` events explain the weights, founding/adoption choice,
policy scores, optional research candidate, current research and reasons to
pause a religious research race. They are diagnostics rather than UI notices.

Normal-turn scenarios `religion-ai-goals` and `religion-ai-coalition` exercise
founding for different goals and adoption for an alliance. Source-adjacent tests
cover the normal research selector, claimed religions, emergency gates, useful
Missionary adoption, hidden beliefs/ownership and economic trade ties.

AI strategy validation: all-target tests pass (1,381 library, 2 utility and
15 CLI tests; 1 ignored), along with Clippy, documentation tests, formatting and
the build without desktop features. The existing `shared-strategy` regression
continues to isolate strong native advisor preferences from optional religious
research. Research selection and goal queries retain deterministic RNG behavior.

Release validation also passes `religion-ai-goals` (one turn, 1/1 checks),
`religion-ai-coalition` (one turn, 1/1), `religion-ai` (two turns, 1/1), and
`shared-strategy` (one turn, 5/5), seed 42, with zero collector drops. The goals
scenario observes science/culture/economy-driven founding; the coalition scenario
observes minority-faith adoption and preservation of the existing alliance.

## Additional AI campaign scenarios

Four registered runtime scenarios extend the founding/coalition checks:
`religion-ai-research-race`, `religion-ai-rival-claim`,
`religion-ai-recovery`, and `religion-ai-missionary`. See SCENARIOS.md for the
prepared research boundary, financial shock, parallel control, required events
and run lengths. Unit coverage uses seeds 42, 7 and 99 and aggregates failures
so every case is evaluated. Scenario observations retain selected goals,
research interest, completed research, founding, government, and comparison-city
influence so a failure can be diagnosed without guessing from a summary.

Additional campaign validation passes all four scenarios in release mode at
seeds 7, 42 and 99 (12/12 runs, no collector drops). The all-target suite passes
1,382 library tests, 2 utility tests and 15 CLI tests, with 1 ignored; Clippy,
formatting and documentation tests pass. The Missionary scenario converts both
tracked cities in five turns while the Tolerant comparison retains both as
mixed. The race and rival-claim scenarios require matching actual research and
founding outcomes; the recovery scenario records the incentive changing from
positive to zero under an observed financial crisis.

Four additional diplomacy scenarios compare otherwise identical campaigns with
different declared faiths or policies: `religion-diplomacy-shared`,
`religion-diplomacy-different`, `religion-diplomacy-tolerant`, and
`religion-diplomacy-war`. Actual alliance offers show shared faith tipping
acceptance, different Devout faiths blocking a marginal agreement, and Tolerant
removing only the receiver's penalty. Normal AI planning at identical raw trust
of -2,290 starts a four-turn Hatred preparation only in the different-faith
campaign; both it and the undeclared control retain Peace. The fixtures test
decision boundaries, not campaign-wide diplomatic balance or war frequency.

All four pass in release mode at seeds 42, 7 and 99 (12/12 runs, no collector
drops), and in the source-adjacent regression. The all-target suite passes
1,383 library tests, 2 utility tests and 15 CLI tests, with 1 ignored. Formatting,
Clippy with warnings denied, and documentation tests also pass. Scenario setup,
observations and documentation are the only additions in this expansion;
religion and diplomacy gameplay rules did not need changes.

### Policy effectiveness review

The unprepared seed-42 campaign demonstrated founding and adoption but exposed
two weaknesses in the evaluator. Missionary valued possible destinations even
when existing sources already supplied the incoming-pressure cap. Devout
estimated current contentment by adding its nominal bonus back to unhappiness,
which can credit content that luxury, martial law or other effects already
cover. Governors can also hide a real content need with entertainers.

Missionary now values marginal incoming pressure after the cap, using all own
dominant sources (including sources outside the bounded city-evaluation sample).
It still uses only public peaceful connections for foreign opportunity and
does not inspect foreign city beliefs. Devout compares the complete native
citizen pipeline with and without its content; an entertainer-release estimate
must leave the candidate assignment out of disorder. Existing mood calculations
use the same native pipeline and actual religion content as before.

The switching threshold now includes `ai.switching_city_cost` per evaluated
city, in addition to the existing base threshold and ten-turn cooldown. Public
partner weights use pact/raw attitude rather than letting the current religious
bonus promote its own diplomacy weight. Decision tracing breaks the score into
alignment, culture, content, defense, spread and diplomacy, records the best
candidate versus selected state, and explains the required improvement.
Alignment remains a small domestic commitment preference; declaring a faith
does not grant its city bonus. Strong native research requests, recovery,
military commitments and actual space programs retain their existing priority.

`religion-ai-culture` (three turns) and `religion-ai-saturated-spread` (eight
turns) check actual accumulated history and influence against parallel controls.
The capped fixture requires five conversions with identical influence while
Tolerant earns more culture. A native-mood regression additionally requires
Devout to solve a real disorder boundary and to yield when that content becomes
redundant. These complement the existing goal, coalition, research, missionary
and diplomacy scenarios rather than requiring every campaign to select every
policy or preserve different national declarations.

Validation passes all 16 religion scenarios at seeds 42, 7 and 99, plus
`shared-strategy` and `proactive-trade` at seed 42 (50/50 release runs, no collector
drops). The culture comparison gains 1,030 history versus 1,021 for Devout over
three turns. The capped-spread comparison gains 180 versus 126 for Missionary
over eight turns, with all five targets converted and identical influence at
every observed boundary. The full suite passes 1,385 library tests (one ignored),
2 utility tests and 15 CLI tests; formatting, Clippy and documentation tests pass.

Two unprepared, all-AI, 200-turn release campaigns at seeds 42 and 7 each found
four faiths naturally. All 1,600 religion decisions have consistent score sums;
every switch clears its improvement threshold and declaration cooldown, with no
collector drops. Seed 42 ends with 34 dominant cities out of 88, and two distinct
declared faiths; declaration/policy changes fall from 26 in the earlier campaign
to 20. Seed 7 ends with 20 dominant cities out of 57 and three declared faiths;
its AI uses Devout for 104 civilization-turns, alongside Missionary and Tolerant.
Observed Devout adoptions value native contentment relief, including cases where
it outweighs a diplomatic penalty. The final AI state is not required to use
every policy simultaneously.

Seed 7 records nine positive optional research opportunities for Code of Laws
while England is researching its Writing prerequisite. Writing was already the
initial project; Code of Laws later completes through normal research on turn
59 and Confucianism is founded that turn. This demonstrates opportunity tracking
and eventual research/founding, but does not establish that the religion incentive
caused the research choice. Native research safeguards remain unchanged.
The campaigns record respectively nine and seven government-advisor warnings,
all resolved; these are heuristic government findings, not religion invariant
failures. These controlled outcomes and two campaigns support functional policy
effectiveness, not optimal play, campaign-wide balance or player enjoyment.

### Integration with current main

The culture-core changes published on main use save format 0x31. The combined
religion format is therefore 0x32, appending religion to the culture-age,
notification and vassalage payload while retaining the published 0x31 loader.
Religion culture is included in `city_culture_per_turn`, so displayed rates,
accumulation and border milestones use the same total alongside building ages
and mobilization. The merged all-target suite passes 1,386 library tests (one
ignored), 2 utility tests and 15 CLI tests; documentation tests, Clippy and
formatting pass.
