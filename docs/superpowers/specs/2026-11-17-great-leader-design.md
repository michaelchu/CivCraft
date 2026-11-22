# Great Leader (Civ III): creation and use

Status: draft for approval. Date: 2026-11-17.

## Intent and why

CivCraft's catalog already has a Civ III `Leader` unit, but nothing creates
it in play and its one behavior is a stub: the "Help Wonder" action in
`src/sim/leaders.rs` sets a city's stock to the item's cost for any building.
Worse, the Leader is buildable from the city screen at 0 shields (see goal
4), so a player can build one every turn and complete any wonder for free.
The project owner wants the Civ III mechanic instead: a rare **Great Leader**
that is born from an elite unit's victory, can be walked into a city, and
completes the city's current improvement or wonder in one turn. The goal is a
game that plays closer to Civ III, where a Great Leader is a prize you plan a
wonder around, and AI civs benefit from them the same way players do.

**Correction made during planning:** an earlier draft said the Leader is
Freeciv's regicide (`GameLoss`) unit and that the AI hunts it. That was wrong.
`freeciv_unit_type_flag(UnitKind::Leader, "GameLoss")` is asserted false, no
unit in the installed roster carries `GameLoss`, `src/sim/ai/hunter.rs` is the
AI's general hunter role, and `leader_paths` in `src/sim/leaders.rs` is general
AI path-finding used by exploration. None of that is Leader-specific, so none
of it is removed by this work.

## Goals

1. Nothing in the game may end a game or civil-war a civ because a Leader
   dies (already true; keep it true).
2. A land-combat victory by an elite unit can create a Leader (odds below).
3. A Leader moved into a friendly city completes that city's current
   building production (improvement, small wonder or Great Wonder) and is
   consumed.
4. AI civs create and use Leaders with a simple, sensible policy.
5. A Leader can never be built: not from the city screen, worklists, the
   AI's production choice, or any other production path. Today it can: the
   catalog record has `cost_shields = 0` and is not flagged no-build, so
   `can_produce` is true for every civ (verified) and the city screen offers
   it at 0 shields, which combined with the existing Help Wonder action lets
   a player finish any wonder for free. Fix: flag the Leader no-build in the
   unit rules (where `NoBuild` is derived in `civ3_unit_rules.rs`) and cover
   it with a test that no civ's city can produce or queue one.
6. Regression coverage: unit tests plus one registered headless scenario.

## Non-goals

- Forming Armies from Leaders.
- Scientific or artistic leaders from technology discovery.
- A Regicide game mode or any rule that ends a game when a unit dies.
- Giving civs a Leader at game start. (Building one is forbidden outright;
  see goal 5.)
- New Leader art (the catalog already has the four era folders).

## Anti-goals (a failure even if it "works")

- A Leader, or its death, ever ending a game or triggering civil war logic.
- Any way to convert a Leader into gold or into shields for a non-building
  item (the Coinage exploit found earlier: Coinage has a nominal cost of 999).
- A Leader being usable in a city that is not the owner's, or by a
  different owner.

## Constraints

- `AGENTS.md` makes Freeciv the authority for gameplay and says Civ III is
  not. This design is an explicit, owner-approved exception for Leader
  creation and use. Record it in `AGENTS.md` and
  `docs/FREECIV_GAMEPLAY_PORT_PLAN.md` as part of this work.
- The Civ III rules come from community write-ups, not official
  documentation. The creation odds are from a 2002 CivFanatics guide for
  patch 1.29f. Sources disagree on whether Conquests military leaders may
  complete Great Wonders; the owner decided: **yes, all wonders and
  improvements**.
- Veteran levels come from the C2C3 ruleset: green, veteran, hardened,
  elite. "Elite" means the top level of the unit's veteran system.
- Combat draws from the simulation RNG. Rolling for a Leader must not change
  the RNG stream for combats where no roll is possible, to limit churn in
  seeded tests and golden outputs.
- Campaign save files store units; adding a unit field needs a schema bump
  with a default for old saves.
- Headless-only builds (`--no-default-features --features scenarios`) must
  keep compiling; no graphics crates may be used.

## Approaches considered

1. **Extend the existing `Leader` unit in place (chosen).** Make it
   unbuildable, add creation in the combat/promotion code, replace the
   unchecked `help_wonder` with a validated completion command, and add AI
   use. Smallest change that gets the Civ III behavior; reuses the catalog
   record, art and UI wiring already present.
2. Add a separate Great Leader unit kind. Rejected: the Civ III catalog has a
   single `leader` record, and two kinds would duplicate art and UI wiring.
3. Also delete Freeciv's generic `GameLoss` support (valuations and the
   city-transfer bounce) as part of this work. Not part of this plan; see the
   optional cleanup below. It is unrelated to the Leader because no unit in
   the roster carries `GameLoss`.

## Decisions (made by the owner)

- Civ III is the model for the Leader's creation and use.
- Leaders complete all wonders and improvements.
- Leaders can never be built.
- Heroic Epic requires the owner to have at least one elite unit (a stand-in
  for Civ III's "a victorious army"; Army forming was originally deferred). Today it has
  no requirement at all and no effect; this work gives it the Leader-odds
  effect and this requirement.

## Decisions (my calls, to be confirmed or corrected)

- **Creation odds (Civ III guide):** per eligible victory, attacking 1 in
  16, 1 in 12 if the owner has the Heroic Epic; defending 1 in 32, 1 in 24
  with Heroic Epic. A unit that has already created a Leader never creates
  another. Zero chance if: the winner is naval, air or a missile; the loser
  belonged to the barbarians; the winner is loaded in an Army; the owner
  already has a Leader.
- **Winner and loser:** only a kill counts. The attacker "wins" when the
  defender dies; the defender "wins" when the attacker dies.
- **Where it appears:** on the winning unit's tile, for the winning owner.
- **Use:** a command, available when a Leader is in a city of its owner, the
  city's production is a building of genus improvement, small wonder or
  great wonder that the city can legally build, and the stock is below the
  cost. It sets the stock to the cost and consumes the Leader. Coinage
  (convert genus) and spaceship parts are excluded. Units are not completed.
- **AI policy:** an AI Leader moves to its wonder city if that city is
  building a wonder, otherwise to the owner's city with the most expensive
  remaining building, then uses the command. Ties go to the nearest city.
- **Documentation:** the `AGENTS.md` and port-plan exceptions above, plus
  `docs/HEADLESS_ANALYTICS.md` for the new lifecycle origin.
- **Persistence:** one new boolean on `Unit`, `created_leader`, defaulting to
  false on load.

## Calls settled after the builder check (my calls)

- **One Leader at a time per owner:** "already has a Leader" means a living
  Leader unit owned by that civ exists. It is a scan of the owner's units at
  roll time, after the cheaper eligibility checks pass. Creation is allowed
  again as soon as the previous Leader is used or dies.
- **A Leader is a noncombat unit** (the catalog has attack 0, defense 0,
  3 moves). It can be killed, never captured: it is not flagged capturable,
  so an enemy entering its tile destroys it instead of taking it, and the
  game never transfers a Leader between owners. AI advisors value it as a
  noncombat unit and never as a defender or attacker.
- **A Leader with nothing to do** (no city of its owner has a legal
  building target) waits, fortified in its safest city. There is no
  timeout; it is used when a target appears.
- **`created_leader` is per unit instance.** It survives upgrades of the
  same unit and disappears when the unit dies or is disbanded. The winner is
  always alive when the roll happens (it won by killing), so there is no
  winner-dies case to handle.
- **Eligible winners:** units with a veteran system that are at its top
  level and whose domain is land. Civilians (single level, no veterancy) are
  never eligible. A land unit attacking from a ship (marines) counts.
- **Use-time validation, then lock:** the command validates legality when
  used. After it sets the stock to the cost, production is locked for that
  turn exactly as a purchase locks it (`bought_production`), so the stock
  cannot be moved to a different item. The normal end-of-turn production
  step then completes the item.
- **Odds live in data, not code:** the four odds and the Heroic Epic
  adjustment go in a small table in the native content TOML next to the
  other AI/settlement tuning, so they can be tuned without code changes.

## Calls settled in the final review (my calls)

- **Eligibility is judged before the fight.** A unit promoted to elite during
  the fight does not create a Leader from that kill.
- **No Leaders from winners at sea.** A marine attacking from a ship would
  spawn a Leader on an ocean tile, so a winner standing on water never creates
  one. This narrows the earlier "marines attacking from a ship count".
- **A Leader is lost, never handed over,** when a city trade, incitement or
  civil war would change its owner.
- **The human player is told** when a Leader appears.

## What changes and what does not

Changes:
- `Leader` gets the no-build flag (derived in
  `src/sim/civ3_unit_rules.rs`, near `F::NoBuild`) and is no longer
  capturable (it currently is, because the capturable rule is attack 0 and
  defense 0). Its veteran status stays "no veterancy" so it never rolls for a
  Leader itself.
- `help_wonder` / `can_help_wonder` in `src/sim/leaders.rs` are replaced by
  the validated completion command described above; the client's "Help
  wonder" button and `Command::HelpWonder` are renamed or reused for it.
- New: creation hook in the combat/promotion code, the `created_leader` unit
  field and save-schema bump, the data table for the odds, AI use, the
  lifecycle origin, one registered headless scenario, and the documentation
  exceptions.

Does not change (checked, not Leader-specific):
- `src/sim/ai/hunter.rs`: the AI's general hunter role.
- `leader_paths` and `ai_movement_cost_order` in `src/sim/leaders.rs`:
  general AI path-finding used by exploration. The module name is misleading;
  renaming it is a possible follow-up, not part of this work.
- The generic `GameLoss` support (`offense.rs`, `advisors/advisor_defenders.rs`,
  `civil_war.rs`, `cities.rs`, `hunter_juiciness`). No unit in the installed
  roster carries `GameLoss`, so it is dormant. Optional cleanup, only if the
  owner confirms: delete it in a separate change.

## Left to the builder

- The exact promotion hook (the combat code that already knows winner,
  loser and veteran level) and how the roll avoids extra RNG draws.
- Naming: the command, UI label ("Complete production") and event text.
- The lifecycle origin for a created Leader and how analytics should show
  it.
- Details of "loaded in an Army" in CivCraft's transport model.
- The registered scenario's setup and checks.

## Sources

- CivFanatics, "Great Leaders: Everything you always wanted to know"
  (2002, patch 1.29f): creation odds, exclusions, completion of
  improvements and wonders.
- Forum threads and the Civilization Wiki page for Leader (Civ3) on whether
  Conquests military leaders can rush Great Wonders; sources conflict, and
  the owner has decided.


## Army formation extension (2026-11-20)

The owner authorized a **Form Army** action for a combat-earned Great Leader.
It requires remaining movement, no transport, and an owned city on the same
tile. Each four owned cities support one living Army. Successful formation
consumes the Leader, creates an Army at that city, assigns that home city,
and caps its movement at the Leader's remaining movement. Rejected commands
consume neither the Leader nor random numbers. The command checks human
ownership; the lower-level legality helper applies to every owner.

Armies are excluded from city production and queues, including Military
Academy cities, at the owner's request. This deliberately differs from the
installed Conquests Civilopedia, which also allows Military Academy production.
The Pentagon's missing prerequisite is restored: three owned Armies in the
field, as recorded under `BLDG_Pentagon` in the installed Civilopedia. Formation
capacity lives in `settlements()` in `src/rules/native.rs`; the Pentagon requirement lives
in `content/buildings.toml`.

The owner subsequently authorized completing Army combat and the Pentagon
capacity bonus. Army cargo is restricted to owned land combat troops; civilians,
bombard-only units, nuclear units and nested Armies cannot join. Membership is
permanent. Three members fit normally; an active owned Pentagon adds one slot.
Losing the Pentagon prevents further loading without ejecting existing members.
Armies can themselves ride compatible naval transports, with their members.

An Army's health is the sum of member health. The healthiest of the strongest
available attackers/defenders fights each round, with stable unit-ID ties.
Existing combat modifiers and firepower apply to that member; another member
continues when it is exhausted. Exhausted members stay in the formation and
recover with it. The last surviving active member can gain ordinary experience;
Army victories do not create Leaders. Defeat removes all members, including in
stack-protected cities. Occupation carries the members into the conquered tile.
Bombardment damages the same pool. Stationary recovery uses the existing recovery
effects against combined maximum health and restores members, rather than
independently healing each cargo unit. Movement uses the slowest member plus one
move (installed Conquests readme); normal action costs allow repeated attacks
while movement remains.

This is a documented CivCraft adapter: the installed Civilopedia establishes
combined-unit combat but does not specify an exact strength-bonus or member
switching formula. No undocumented Conquests strength bonus is invented. The
existing AI Great Leader building-completion preference remains in place.
Member identities and health use existing saved cargo records; maximum Army
health is a derived cache rebuilt on load, including old campaign layouts.

Validation: `leader-army` exercises real formation commands, Leader consumption,
the city limit, and the Pentagon unlock; `army-combat` exercises loading, the
fourth slot, combined health, combat, and occupation through player commands.
`great-leader` covers existing AI use.
Source-adjacent tests cover rejected actions, ownership, movement, production
and queue rejection, loss of a qualifying Army, and save round trips. The
`leader-form-army` UI review scene clicks the action and checks selection;
`army-loaded` verifies the Pentagon capacity in the unit context.


### Army combat presentation (2026-11-20)

The owner requested regular battle animations for the members selected by the
combat resolver. When visual observation is enabled, a battle records one
exchange per continuous pair of selected members. Each exchange snapshots the
actual member's type and individual health before damage, then uses the existing
regular attack/defend clips, source timings, facing, health display and sounds.
Visual proxies retain their carrier's map ID so playback hides the Army flag
and does not reveal other cargo or draw duplicate troops. Snapshots survive
member/Army removal and the carrier returns normally when playback finishes.
Member exhaustion can play the member's Death clip, but the opponent does not
celebrate victory between members. Only the final exchange celebrates the
complete battle's result. Disabled visual events add no member snapshots and
consume no random numbers; tests compare campaign bytes and random state.

Native scenes `army-attack-swordsman`, `army-attack-horseman`,
`army-attack-spearman`, `army-defend-swordsman`, `army-defend-spearman`, and
`army-defend-horseman` inspect both roles across all three members. Ordinary
`combat-animation-frames` is compared against a capture of the prior executable.
