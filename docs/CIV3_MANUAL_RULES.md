# Civilization III manual rules catalog

Status: **Terrain entries started.** The entries below capture the water and
special land terrain facts used in the current map model. A broader
manual-to-code audit remains outstanding.

## Authority and interpretation

- Gameplay mechanics, formulas, and rule interpretation are governed by Freeciv
  (pinned `civ2civ3` baseline), not by Civilization III. Entries in this catalog
  that describe Civ III mechanics are reference records of the original game and
  should be read alongside the Freeciv mechanic they inform; record any
  difference in the port plan's ledger rather than silently overriding either.
- The supplied manual is the authority for Civilization III content: units,
  buildings, civilizations, and their stated values and identities. Record the
  manual page for every content fact. Include a short quotation only when
  necessary; otherwise paraphrase the relevant passage.
- Do not infer an exact formula, timing detail, exception, or random procedure
  from a general description. Record what the manual says and mark missing
  detail as unspecified.
- If a rule depends on an edition, expansion, difficulty, or game setting,
  record that scope. Do not silently mix base-game and expansion behavior.
- When the manual does not specify a detail needed for an expected result, use
  a clearly identified Civ III-specific supplementary source if available.
  Prefer original game materials and official data, then well-sourced
  specialist references. Record the exact claim each source supports and any
  disagreement between sources.
- Freeciv behavior is the expected result for gameplay mechanics. It is not
  Civ III evidence for content values, and Civ III sources do not override it
  for mechanics. Existing tests and implementation details are neither source.

## Source records

Keep one entry per source in the source list. Link each rule to the source
records that support it. Preserve edition, release, and expansion scope; a
source that mixes versions must be checked before its data is used.

```text
Source ID:
Title / publisher or author:
URL or local file:
Source type: Original game material | Official publication | Specialist reference | Other
Edition / expansion / patch scope:
Pages, table, figure, or section:
Retrieved or checked date:
Claims this source supports:
Limitations or conflicts:
```

### CIV3-MANUAL

- Source ID: CIV3-MANUAL
- Title / publisher: *Civilization III: The Official Manual*, Firaxis Games
- URL or local file: [`civ3_manual.pdf`](civ3_manual.pdf), docs folder
- Source type: Original game material
- Edition / expansion / patch scope: Base Civilization III manual, 2001
- Pages, table, figure, or section: p. 43–44, diplomacy changes and absence of
  Diplomat/Spy units; pp. 138–139, embassies and diplomatic actions; pp. 148–149,
  espionage and international incidents; p. 80, Hit Points and Damage; p. 83,
  Bombardment, Air Superiority, and Fortresses; p. 110, bombardment damage to
  improvements; pp. 96–97, Nationalism and mobilization; pp. 206–207, terrain
  yield and resource tables
- Retrieved or checked date: 2026-11-14
- Claims this source supports: Coast, Sea, and Ocean base yields and resource
  habitats; Flood Plains and shield yielding Grassland yields; new units
  generally have 3 hit points, Veterans 4, and Elites 5 (p. 80); bombardment
  is a ranged attack that can damage units, defensive fortifications,
  population, and city improvements without moving the attacker (p. 83); Air
  Superiority can intercept enemy air units entering its defensive radius (p.
  83); Coastal Fortresses can fire on passing enemy ships and units stationed
  in Fortresses can take free shots at passing enemies (pp. 83–84); city
  improvements can be lost to bombardment (p. 110).
- Supplementary claims: Civilization III has no moving Diplomat or Spy units;
  embassies provide diplomatic actions and city intelligence after Writing,
  and espionage missions require Espionage plus the Intelligence Agency and
  are initiated through embassies (pp. 43–44, 138–139, 148–149). Trade routes
  and resource supply/demand are handled through diplomatic trade agreements
  in this edition (p. 44). Nationalism allows mobilization, including drafting
  citizens to create military units (pp. 96–97); this supports binding the
  C2C3 Conscription ruleset advance to Civ III Nationalism for the draft-related
  tech identity.
- Limitations or conflicts: Does not specify procedural map-generation
  thresholds, the frequency of shielded Grassland, or Conquests-only Volcano
  and Marsh rules. The manual says "generally" for starting hit points and
  does not enumerate Conquests unit-specific hit-point bonuses. For mechanics
  explicitly ported from C2C3 in other scopes, Freeciv remains implementation
  context for those ported formulas and state transitions. Espionage targets
  Conquests; its undocumented formulas remain supplementary or unresolved as
  recorded in `CIV3_ESPIONAGE_DIPLOMACY_CITY_ACTIONS_DESIGN.md`. The
  Conscription-to-Nationalism mapping preserves the Civ III research tree; it
  does not claim the C2C3 prerequisites or all effects of the two advances are
  identical.

- Source ID: CIV3-COMPLETE-MANUAL
- Title / publisher: *Civilization III Complete Manual*, Firaxis Games / 2K Games
- URL or local file: <https://steamcdn-a.akamaihd.net/steam/apps/3910/manuals/manual.pdf?t=1569013660>
- Source type: Official publication
- Edition / expansion / patch scope: Civilization III Complete, including Conquests
- Pages, table, figure, or section: Tutorial, PDF p. 29; Settler cost, PDF p. 34
- Retrieved or checked date: 2026-11-15
- Claims this source supports: Settler production costs two population; the tutorial advises accounting for city growth to size 3 before the Settler completes, after which the population falls by two.
- Limitations or conflicts: This manual does not specify ongoing food upkeep for Settlers or an AI production desirability rule. The food-surplus gate in CivCraft is explicitly an imported Freeciv/Civ2Civ3 AI heuristic, not a Civ III rule.

- Source ID: PRIMA-CIV3-GUIDE
- Title / publisher: *Prima's Official Strategy Guide*, David Ellis, Prima Games
- URL or local file: <https://device.report/m/d76e460361da693c0ec91ddff041d42d7a117f67b42cd55f945543fbd819c27a.pdf>
- Source type: Official publication
- Edition / expansion / patch scope: Base Civilization III, 2001–2004 printing
- Pages, table, figure, or section: printed p. 174, "Hit points for undamaged units"
- Retrieved or checked date: 2026-11-09
- Claims this source supports: Conscript, Regular, Veteran, and Elite units have 2, 3, 4, and 5 HP; experience rank affects HP but not offensive or defensive capability.
- Limitations or conflicts: The guide describes base Civilization III. Conquests BIQ unit-specific hit-point bonuses remain the target source for Conquests exceptions.

- Source ID: PRIMA-PTW-STRATEGY-GUIDE
- Title / publisher: *Civilization III: Play the World Advanced Strategies*, Prima Games
- URL or local file: <https://oldapplestuff.com/download/Macintosh/Macintosh_Garden/manuals/Civilization_III_PTW_strategy_guide.pdf>
- Source type: Official publication
- Edition / expansion / patch scope: Play the World expansion strategy guide; its introduction states that, unless an item is marked PTW-only, strategies also apply to original Civilization III
- Pages, table, figure, or section: printed pp. 108–110, “Those Wonderful Workers” and “The Workers you capture”; pp. 113–116, “Bombardment and Bombing”; p. 168, building quick-reference table
- Retrieved or checked date: 2026-11-14
- Claims this source supports: captured foreign Workers complete jobs at half the rate of the owner's native Workers; stacking Workers combines their progress rates. The bombardment section gives city target weights of 25% population, 25% buildings, and 50% units; gives buildings, citizens, and terrain improvements 16 base defense and 1 HP, modified by applicable defense bonuses; and documents terrain-improvement destruction priority. The air-combat section identifies Fighters, Jet Fighters, F-15s, and Stealth Fighters as interceptors, with 50% detection of ordinary missions, 5% of stealth missions, and one shot per successful detection per turn. The quick-reference table gives Coastal Fortress naval bombardment defense 8 and says it automatically bombards passing enemy ships.
- Limitations or conflicts: This is a PTW-era guide, not a Conquests manual. Its bombardment target probabilities and object order are supplementary evidence and have not been confirmed against the Conquests executable. Conquests PRTO `WorkerStrength` remains authoritative for the editable source value, and the installed campaign value (1.0 for Worker) is interpreted against the native worker progress baseline of 2.

- Source ID: CIVFANATICS-CIV3-FAQ
- Title / publisher: "Civilization III: Frequently Asked Questions," CivFanatics
- URL or local file: <https://civfanatics.com/civ3/faq/>
- Source type: Specialist reference
- Edition / expansion / patch scope: Mixed Vanilla, Play the World, and Conquests; individual claims below note scope
- Pages, table, figure, or section: Warfare, unit experience, promotion odds, and bombard rules
- Retrieved or checked date: 2026-11-09
- Claims this source supports: Regular units are the default; units built in cities with Barracks are Veterans; promotion odds are 1/2, 1/4, and 1/8 by rank, doubled for Militaristic civilizations and halved against Barbarians; bombard uses explicit tile range and can target cities and terrain improvements.
- Limitations or conflicts: Community reference with mixed-version content. Use the installed Conquests BIQ for campaign values and verify expansion-specific behavior separately. The FAQ also states a same-turn repeat-battle promotion guarantee that the current engine does not yet model.

- Source ID: CIVILIZATION-WIKI-BOMBARD
- Title / publisher: "Bombard (Civ3)," Civilization Wiki
- URL or local file: <https://civilization.fandom.com/wiki/Bombard_(Civ3)>
- Source type: Specialist reference
- Edition / expansion / patch scope: Civilization III, with Conquests-specific lethal bombard flags noted
- Pages, table, figure, or section: Mechanism; Rate of fire; Defensive bombard
- Retrieved or checked date: 2026-11-09
- Claims this source supports: Bombard Strength is compared against modified unit defense; each shot in the rate of fire is applied to the same selected defender; ordinary bombard is nonlethal unless the applicable lethal land/sea flag is set.
- Limitations or conflicts: Community wiki. The BIQ PRTO fields and flags are the authoritative values for this checkout; the general target-selection and city/improvement effects still need source-level verification.

- Source ID: CIV3-CONQUESTS-MANUAL
- Title / publisher: *Civilization III: Conquests*, Firaxis Games
- URL or local file: [online manual](https://manualmachine.com/gamespc/sidmeierscivilizationiiiconquests/1118672-user-manual/)
- Source type: Original game material
- Edition / expansion / patch scope: Civilization III: Conquests
- Pages, table, figure, or section: pp. 30–31, “Automated Bombard, Bombing,
  and Precision Bombing”; pp. 36–39, “Airfield” and “Radar Tower” worker
  actions; “Clear Damage” worker action; p. 83, “Air Battles,” “Bombardment,”
  and “Fortresses”
- Retrieved or checked date: 2026-11-14
- Claims this source supports: Automated bombard, bombing, and precision
  bombing repeatedly act on one target without a new order each turn; Auto
  Precision Bombing targets city improvements only (pp. 30–31). Workers can
  build Airfields after Flight; an Airfield can host air units for re-basing
  and air operations (pp. 36–37). Bombardment can cause craters; Workers clear
  them with the Clear Damage command (pp. 38–39). Air Superiority units can
  intercept enemy air missions entering their defensive radius, and Coastal
  Fortresses fire on passing enemy ships (p. 83). Bombardment can affect units,
  fortifications, population, and city improvements (p. 83).
- Supplementary claim: The selected C2C3 Airstrip/Airbase requirements name
  Freeciv Radio, which is absent from the Conquests research catalog. The
  campaign adapter binds those requirements to Civ III Flight, matching the
  manual's Airfield prerequisite without adding Radio as a researchable
  advance.
- Limitations or conflicts: The manual does not specify which unit field enables
  craters, the exact creation probability, or the yield penalty. It does not
  specify target selection probabilities, Air Superiority interception chance,
  or the detailed Coastal Fortress firing procedure; those remain unresolved
  for Conquests and are not inferred from Freeciv.

- Source ID: CIV3-CONQUESTS-CIVILOPEDIA
- Title / publisher: *Conquests* installed Civilopedia text, Firaxis Games
- URL or local file: [`../../source/Conquests/Text/Civilopedia.txt`](../../source/Conquests/Text/Civilopedia.txt), “Craters” entry
- Source type: Original game material
- Edition / expansion / patch scope: Civilization III: Conquests
- Pages, table, figure, or section: “Craters,” lines 311–320; “Espionage,” lines 817–928; “Embassies,” lines 1607–1620; “Intelligence Center,” lines 5762–5765; “Espionage” technology, lines 6535–6539
- Retrieved or checked date: 2026-11-14
- Claims this source supports: Craters can form in non-city squares when bombardment eliminates the last unit or terrain improvement; they reduce food, production, and commerce by one, and Workers can clear them. In Conquests, Writing opens the Espionage screen; the Espionage technology and Intelligence Agency enable espionage; the screen tracks embassies, planted Spies, experience, targets, mission choices, and costs; and the catalog includes city investigation, technology/map/plans theft, propaganda, counterspy, and production sabotage.
- Limitations or conflicts: “May be formed” does not state an exact random chance. The installed Civilopedia describes the crater effect but not its PRTO flag mapping; the installed BIQ supplies `CreateCraters` values. Espionage outcomes and formulas are not specified in the Civilopedia entries summarized here.

- Source ID: APOLYTON-BIQ-FORMAT
- Title / publisher: "Civilization III BIC file format (2nd thread)," Apolyton
  Civilization Site forum
- URL or local file: <https://apolyton.net/forum/civilization-series/civilization-iii/59815-civilization-iii-bic-file-format-2nd-thread/page3>
- Source type: Specialist reference
- Edition / expansion / patch scope: BIQ format used by Conquests
- Pages, table, figure, or section: PRTO unit ability flags
- Retrieved or checked date: 2026-11-09
- Claims this source supports: PRTO ability flag byte 4 bits 3 and 4 represent Lethal Land Bombardment and Lethal Sea Bombardment.
- Limitations or conflicts: Forum reverse-engineering notes; the decoded values are cross-checked against installed campaign unit records and known unit behavior.

- Source ID: CIVFANATICS-ESPIONAGE-STUDY
- Title / publisher: “A Study of Espionage Missions,” CivFanatics; author attribution is not shown in the article body
- URL or local file: <https://civfanatics.com/civ3/strategy/game-mechanics/a-study-of-espionage-missions/>
- Source type: Specialist community analysis, reverse engineered
- Edition / expansion / patch scope: Civilization III edition and patch are not pinned by the article
- Pages, table, figure, or section: “Prices of Espionage Missions,” “Success chance,” “Government modifier,” and “Culture factors”
- Retrieved or checked date: 2026-11-14
- Claims this source supports: Base operation cost equations; embassy/city investigation success; Spy planting rates and the failed-plant retry marker; mission escape chances and the separate 80% operation-success roll; propaganda citizen persuasion formula, Democracy immunity, government modifiers, culture categories, 75% city-flip threshold, and confused-citizen effects.
- Limitations or conflicts: Community reverse engineering, not official documentation; the edition and patch are unspecified. The article says Carefully costs 50% extra by wording that differs from CivWiki's explicit 1.5x. Confirm values against Conquests 1.22 before asserting exact parity.

- Source ID: CIVWIKI-CIV3-ESPIONAGE
- Title / publisher: “Spionageaktionen (Civ3),” CivWiki
- URL or local file: <https://civ-wiki.de/wiki/Spionageaktionen_%28Civ3%29>
- Source type: Specialist community summary citing Prima and community analysis
- Edition / expansion / patch scope: Civilization III generally; no Conquests patch is pinned
- Pages, table, figure, or section: “Kosten der Spionageaktionen,” “Erfolgswahrscheinlichkeiten,” and “Propaganda”
- Retrieved or checked date: 2026-11-14
- Claims this source supports: Mission base costs and risk multipliers; embassy versus Spy escape and success chances; veteran Spy planting in Communism/Fascism; failed-plant retry chance; propaganda veteran/nationality modifiers, government/culture factors, and confused-citizen behavior.
- Limitations or conflicts: Secondary summary with no patch-specific validation. Its Carefully modifier is 1.5x, conflicting with the CivFanatics article wording. Its claims are supplementary, not manual-derived rules.

## Unit mechanics

### Foreign Worker progress

- Expected rule: only units with Worker terrain actions can improve terrain.
  A captured Worker keeps its former nationality and works at half the rate of
  a native Worker. Capturing a Settler yields two Workers with the captured
  nationality, so they use the foreign Worker rate too. Settlers themselves do
  not improve terrain. Stack progress is additive.
- Sources: Complete manual, pp. 56–57 and 112; Prima's *Play the World
  Advanced Strategies*, pp. 108–110; installed Conquests BIQ `PRTO::WorkerStrength`
  retained in `units.tsv` and documented by the adjacent OpenCiv3 save parser.
- Scope: standard Conquests campaign. `WorkerStrength` is 1.0 for Worker,
  Crusader, and TOW Infantry. Worker is the standard campaign Worker unit.
  Crusader has the separate Build Fortress action but cannot be captured or
  traded in the standard campaign; TOW Infantry has no worker action flags.
  Their raw values do not apply the foreign Worker progress rule.
- Implementation: `movement/activities.rs::activity_rate` applies the table
  value to foreign Worker progress relative to the native baseline of 2.
  The same rate feeds actual activity progress and projected completion.
- Remaining gap: campaign WorkerStrength is only 1.0 for action-capable
  Workers. Runtime supports source values through the existing integer work
  accumulator; sub-integer progress precision for arbitrary fractional
  rebalances has not been verified.

### Unit treasury support

- Expected rule: units marked `RequireSupport` consume one free-support slot;
  additional eligible units cost the current government's gold rate each turn.
  Free slots are summed across towns (size 1–6), cities (7–12), and
  metropolises (13+). Settlers are included. Unit support consumes neither
  food nor shields. Settler production costs two population; Worker production
  costs one.
- Source: Complete manual, pp. 38–39, “No home city,” “Paying for support,”
  and “Settlers and Workers”; Conquests government support table at
  <https://civilization.fandom.com/wiki/Government_(Civ3)>; Conquests FAQ,
  “Unit Costs,” at <https://civfanatics.com/civ3/faq/>.
- Scope: standard Conquests campaign. Government allowances and gold rates are
  the Conquests values; the manual establishes the treasury model and
  population costs for Settlers and Workers.
- Implementation: `government_unit_support.tsv` stores the editable
  town/city/metropolis allowances and excess-unit rates. `units.tsv` provides
  `require_support` and `population_cost`. Simulation support costs are
  treasury gold; no food or shield support is charged.
- Government identity/availability: the active roster now uses Anarchy,
  Despotism, Monarchy, Communism, Fascism, Feudalism, Republic, and Democracy.
  Despotism is the default; Fascism requires the Fascism advance (Nationalism
  and Industrialization prerequisites), and Feudalism requires Feudalism.
- Remaining gap: non-support government effects still need a source-backed
  Conquests audit. The old field-unit unrest mechanic was removed; Civ III war
  weariness is a separate system and is not implemented by this change.

### Military police and government happiness

- Expected rule: a military unit with attack strength at least 1 stationed in
  a city makes one unhappy citizen content, up to the government's per-city
  Military Police limit. Conquests limits are 2 for Despotism, 3 for Monarchy
  and Feudalism, 4 for Communism and Fascism, and 0 for Anarchy, Republic, and
  Democracy. Units in the field or at a base do not cause home-city unrest.
- Sources: *Civilization III: The Official Manual*, Chapter 11, “Restoring
  Order” (military police); Conquests Complete v1.22 Civilopedia government
  table transcribed at <https://civfanatics.com/civ3/infocenter/> (government
  military-police limits); CivFanatics Civ III FAQ, “Governments,” at
  <https://civfanatics.com/civ3/faq/> (Fundamentalism removed; Republic and
  Democracy use war weariness in place of individual unit unhappiness).
- Scope: standard Conquests government roster, not scenario-only governments.
- Implementation: city happiness applies the government limit to garrisoned
  units whose editable attack stat is positive. Legacy field-unit unrest,
  Fundamentalism tithes, and the retired Federation government bonus are not
  used.
- Limitations: event-based war weariness is still not implemented; this entry
  does not imply that those event effects are modeled.

### Unit hit points

- Expected rule: campaign units use the editable experience-level table for
  Conscript/Regular/Veteran/Elite base HP (2/3/4/5); each Conquests unit's
  `hit_point_bonus` from its BIQ record is added to that base. Rank does not
  change attack or defense power.
- Source: CIV3-MANUAL, p. 80; PRIMA-CIV3-GUIDE, p. 174; Conquests campaign BIQ
  `EXPR` and `PRTO` records.
- Scope: the manual and strategy guide describe base Civilization III;
  Conquests rank values and unit-specific bonuses come from the installed
  campaign BIQ.
- Implementation: `experience_levels.tsv` and `units.tsv` are the editable
  data source. Unit creation, promotion, healing, and combat use rank-specific
  maximum HP. Army aggregate HP and other rank-sensitive paths still need
  auditing.

### Combat promotion chances

- Expected rule: a winning unit's promotion chance depends on its current
  rank; the FAQ reports 50%, 25%, and 12.5% from Conscript through Elite.
  Militaristic civilizations double those chances and victories against
  Barbarians halve them.
- Source: CIVFANATICS-CIV3-FAQ, Warfare, "What are the chances my unit will be
  promoted to a higher level?" and the Militaristic trait description.
- Scope: specialist reference spanning releases; installed Conquests
  civilization traits and campaign data define the active campaign.
- Implementation: rates are stored as per-mille data in
  `experience_levels.tsv`; the player's Militaristic trait and Barbarian
  opponent modify the promotion roll in the simulation.
- Unresolved: the FAQ says an unpromoted winner is guaranteed promotion if it
  survives another battle in the same turn. That turn-level rule is not yet
  implemented or verified against the Conquests executable.

### Ranged bombard

- Expected rule: a bombard-capable unit can target another tile within its
  source range without moving. Bombard Strength is compared with modified
  defender defense for each shot; Rate of Fire repeats against the same
  defender. Lethal land/sea bombard flags permit reducing a unit to zero HP.
- Source: CIV3-MANUAL, p. 83; CIVFANATICS-CIV3-FAQ, Warfare,
  "What are ‘bombardment’ units and how do they work?"; CIVILIZATION-WIKI-BOMBARD;
  Conquests BIQ `PRTO` range, strength, rate, and ability flags.
- Scope: standard Conquests campaign, using the installed BIQ values.
- Implementation: editable Civ III unit records now determine Bombard
  capability, strength, rate of fire, per-unit tile range, aircraft operational
  range, lethal land/sea outcomes, and Bombard Fx presentation. A direct
  aircraft bombard action requires a refueling site or carrier. AI aircraft
  can bombard targets from a known refueling site. Under selected C2C3 rules,
  rate of fire reduces
  as a unit loses HP, with a minimum of one shot; target
  damage uses the selected ruleset's per-shot combat roll. Each defender must
  be visible and reachable under the selected C2C3 class rules.
- Freeciv implementation context: the pinned C2C3 ruleset sets
  `damage_reduces_bombard_rate = TRUE`, has adjacent action range, and resolves
  one rate of fire against each reachable defender on the tile. CivCraft uses
  Civ III per-unit range instead of the ruleset's adjacent-only action range.
- AI: Freeciv checks capture/ransom before bombard. CivCraft does the same for
  conventional bombard units, trying an in-range shot before movement and
  scoring candidate tiles by expected defender damage, unit value,
  lethal-kill potential, citizen losses, and known hostile terrain
  improvements. Fuelled aircraft can bombard hostile units and improved tiles
  from a known refueling site. AI aircraft can route across turns to a known
  refueling site that brings a worthwhile target into range. Missions do not
  simulate an automatic return flight after bombardment.
- City bombardment: CivCraft applies the PTW guide's 25% population, 25%
  improvement, and 50% garrison target weights. City objects have one hit point
  and defense 16 with terrain, city-size, Walls, and Coastal Defense modifiers;
  hits can remove one citizen or an ordinary city improvement. Empty terrain
  targets use the guide's defense 16 and improvement priority. These details
  are supplementary PTW evidence, not confirmed Conquests probabilities.
- AI bombard units re-evaluate targets each turn. The player bombard selection
  outlines legal target tiles in range. Auto Bombard repeats for aircraft from
  a refueling site, but flight out and return are not simulated. Precision
  Bombing is available to Stealth Fighters and Stealth Bombers after Smart
  Weapons; it targets city improvements, then population when none remain, and
  can repeat automatically. The implementation chooses randomly among ordinary
  improvements; the manual does not specify the choice rule. AI aircraft select
  it for eligible cities. Air Superiority orders persist at a base and use the
  guide's detection chances, half-range radius, air combat stats, and one shot
  per successful detection. Coastal Fortress makes a strength-8 reaction roll
  against an adjacent enemy ship; the manual confirms the shot and the guide
  provides strength 8. Defensive bombard fires once per unit turn from a
  bombard-capable unit on the attacked tile or from a unit in a nearby Fortress;
  exact Conquests defensive eligibility and reaction timing remain unverified.

### Bombard visual effects and craters

- Expected rule: PRTO `BombardEffects` enables an explosion effect on bombard;
  `CreateCraters` allows bombardment damage on a non-city tile when the last
  unit or terrain improvement there is eliminated. Craters reduce food,
  production, and commerce by one and Workers can clear them.
- Source: Conquests BIQ PRTO `BombardEffects` and `CreateCraters`; CIV3-CONQUESTS-MANUAL,
  pp. 38–39; CIV3-CONQUESTS-CIVILOPEDIA, “Craters”; CivFanatics “Editor Rules - Explained,”
  “Bombard Fx” and “Create Craters” entries.
- Implementation: `BombardEffects` drives a target explosion overlay through
  the existing combat event. Empty-tile bombardment can destroy terrain
  improvements. The source `CreateCraters` flag is loaded, and tile crater state
  already affects yield and can be cleared, but bombardment does not currently
  create craters. City improvements can now be destroyed by a successful city
  bombardment target roll. The
  Conquests Automated Bombard order repeats its selected target each turn and
  survives save/load.
- Unresolved: the Civilopedia says a crater "may be formed" when bombardment
  removes the last unit or terrain improvement; the checked source does not
  specify the probability or exact trigger ordering. `CreateCraters` is
  permission for eligible units to cause craters, but the chance remains
  unknown. The visual effect is a native overlay, not playback of the original
  unit-specific bombard animation.

### Technology-tree source leads

These are candidate references for details not fully tabulated in the
supplied manual. Their claims have not yet been reconciled into rule entries.

- **TECH-WIKI — Vi.ki, “Civ 3 Tech Tree: All Advances, Eras and Optional
  Techs”**: <https://civ3.vi.ki/technologies>. The page says its base-game
  counts come from Prima's official strategy guide and cross-references both
  the 2001 manual and the Complete manual. Treat it as a sourced secondary
  reference; verify table details and version scope against the cited source
  where possible.
- **TECH-CF-TREE — CivFanatics, “Civilization III: Tech Tree”**:
  <https://civfanatics.com/civ3/tech-tree/>. An interactive era-by-era tree;
  verify the displayed version and capture prerequisite details when
  transcribing it.
- **TECH-CF-PDF — Nethog's Civ 3 Tech Tree, Rev 1.00 (29 Oct 2002)**:
  <https://www.civfanatics.net/downloads/civ3/reference/civ3_tech_tree_rev100.pdf>.
  A diagram with technology nodes and prerequisite arrows. Its version and
  expansion applicability must be checked before using it as a base-game
  oracle.
- **TECH-CF-INFO — CivFanatics Civilization III Info Center**:
  <https://civfanatics.com/civ3/infocenter/>. A broad community index that
  gathers information from mixed source types; use as a discovery aid, then
  trace gameplay claims to a more specific source.

### Era progression

- **CIV3-COMPLETE-MANUAL — *Civilization III: Complete* manual**, Firaxis
  Games, [Steam-hosted manual PDF](https://steamcdn-a.akamaihd.net/steam/apps/3910/manuals/manual.pdf?t=1531947942).
  Official manual; Complete edition. Checked 2026-11-13, "Advances" and
  "Optional Advances," lines corresponding to printed pp. 166–167. The manual
  says civilizations must gain all critical technologies of an age before
  researching advances in the next age, and lists optional advances that are
  exempt. It does not enumerate the Conquests-only BIQ's critical flags; the
  checked-in technology catalog supplies that classification.

Rule: an advance from a later era cannot be researched until all critical
advances in the preceding eras are known. Optional advances are exempt.
Technologies gained by trade or minor tribes count as known. A research goal
path must include any remaining critical advances before entering its era.
Supported by `CIV3-COMPLETE-MANUAL`; the critical/optional classification is
from the checked-in Conquests technology catalog and remains subject to BIQ
verification.

The online sources do not fully agree on the reported base-game technology
count: the Vi.ki page reports 82 based on its count of Prima's table and also
mentions an 81-tech count in a Firaxis comparison; CivFanatics reports 82.
Record this as a source discrepancy to resolve during the technology audit,
not as a settled manual-derived fact.

### General gameplay-mechanics index

- **VI-PAGES — Vi.ki, “Every page — Civilization 3”**:
  <https://civ3.vi.ki/pages>. This directory links detailed pages for core
  mechanics including cities, combat, corruption, culture, diplomacy,
  governments, resources, terrain/workers, units, technologies, and victory.
  Treat it as a secondary discovery index: the linked articles cite a mixture
  of manuals, Prima's guide, previews, and archived official material. Follow
  the article's source links and cite the specific underlying source for each
  rule where possible; check base-game versus expansion scope.
- **VI-COMBAT — Vi.ki, “Combat”**:
  <https://civ3.vi.ki/combat>. A worked mechanics summary with a source list;
  use it to locate underlying sources, and validate detailed formulas against
  those sources before treating them as expected values.
- **VI-CITIES — Vi.ki, “Cities”**: <https://civ3.vi.ki/cities>. A city
  mechanics summary whose claims cite specific manual editions and Prima page
  ranges; preserve those edition distinctions when following a claim.
- **CIVFANATICS-ESPIONAGE-STUDY — “A Study of Espionage Missions”** and
  **CIVWIKI-CIV3-ESPIONAGE — “Spionageaktionen (Civ3)”**: supplemental leads
  for reverse-engineered mission formulas; see the source records above and
  the scoped implementation note in
  [`CIV3_ESPIONAGE_DIPLOMACY_CITY_ACTIONS_DESIGN.md`](CIV3_ESPIONAGE_DIPLOMACY_CITY_ACTIONS_DESIGN.md).

## Confidence labels

- **Explicit:** The manual directly states the behavior, value, condition, or
  relationship needed for the claim.
- **Inferred:** The claim follows from the manual's description, but some
  details are not stated. Limit tests to the observable consequence that is
  supported by the text and explain the inference.
- **Unspecified:** The manual does not provide enough detail to set an expected
  result. Do not create a parity assertion for this claim.

## Rule entry template

Copy this block for each distinct rule. Keep one independently testable
behavior per entry.

```text
Rule ID:
Subsystem:
Rule title:
Manual edition / scope:
Manual page(s):
Manual statement (paraphrase):
Confidence: Explicit | Inferred | Unspecified
Supported expected behavior:
Details the manual leaves unspecified:
Supplementary source ID(s):
Related manual rules:
```

## Rule entries

### TERRAIN-WATER-OUTPUTS

- Subsystem: Terrain and city tile output
- Rule title: Coast, Sea, and Ocean base yields
- Manual edition / scope: Base Civilization III
- Manual page(s): 206
- Manual statement (paraphrase): Coast yields 1 food and 2 commerce; Sea yields
  1 food and 1 commerce; Ocean yields 1 food and no commerce. These terrains
  yield no base shields.
- Confidence: Explicit
- Supported expected behavior: Model distinct Coast, Sea, and Ocean terrain
  types with base yields `(1, 0, 2)`, `(1, 0, 1)`, and `(1, 0, 0)`.
- Details the manual leaves unspecified: Map-generation depth thresholds and
  how the generator chooses each water type.
- Supplementary source ID(s): None
- Related manual rules: TERRAIN-WATER-RESOURCES

### TERRAIN-WATER-RESOURCES

- Subsystem: Resource placement
- Rule title: Fish and Whale water habitats
- Manual edition / scope: Base Civilization III
- Manual page(s): 207
- Manual statement (paraphrase): Fish may appear on Coast, Sea, and Ocean;
  Whales may appear on Sea and Ocean.
- Confidence: Explicit
- Supported expected behavior: Permit Fish on Coast, Sea, and Ocean, and
  Whales on Sea and Ocean. Deep Ocean is not excluded.
- Details the manual leaves unspecified: Relative spawn frequency and placement
  spacing.
- Supplementary source ID(s): None
- Related manual rules: TERRAIN-WATER-OUTPUTS

### TERRAIN-FLOOD-PLAINS

- Subsystem: Terrain and city tile output
- Rule title: Flood Plains yield
- Manual edition / scope: Base Civilization III
- Manual page(s): 206
- Manual statement (paraphrase): Flood Plains produce 3 food before
  improvements.
- Confidence: Explicit
- Supported expected behavior: Model Flood Plains as a 3-food terrain variant.
- Details the manual leaves unspecified: The terrain table does not specify a
  map-generation algorithm or placement frequency.
- Supplementary source ID(s): None
- Related manual rules: None

### TERRAIN-GRASSLAND-SHIELD

- Subsystem: Terrain and city tile output
- Rule title: Shield yielding Grassland variant
- Manual edition / scope: Base Civilization III
- Manual page(s): 37, 206
- Manual statement (paraphrase): Some Grassland squares have a rocky-tuft symbol
  and produce one shield in addition to the normal 2 food. The terrain table
  also lists +1 food from irrigation, +1 shield from mining, and +1 commerce
  from roads on Grassland.
- Confidence: Explicit
- Supported expected behavior: Represent shield yielding Grassland separately
  from ordinary Grassland and give it 2 food and 1 shield. Apply the normal
  Grassland irrigation, mine, and road bonuses to this variant.
- Details the manual leaves unspecified: How often map generation places this
  variant and the exact sprite used for its rocky-tuft symbol.
- Supplementary source ID(s): None
- Related manual rules: None

### DIPLOMACY-ESPIONAGE-MANUAL

- Subsystem: Diplomacy and espionage
- Rule title: Embassies and covert missions are UI-driven player actions
- Manual edition / scope: Base Civilization III manual; Conquests interface cross-check
- Manual page(s): Base manual printed pp. 138–139 and 148–149; Conquests Complete manual Chapter 13, pp. 166–167
- Manual statement (paraphrase): Contact and Writing permit embassies; embassies enable diplomatic city investigation and technology theft, close during war and reopen with peace. Covert mission discovery is distinct from operation success and can cause an international incident or be treated as an act of war. The Espionage screen selects opponent, mission, optional target city, and risk.
- Confidence: Explicit
- Supported expected behavior: Keep embassy and Spy as diplomatic assets, offer missions through the Espionage UI, and model detection separately from mission success. Do not add moving Spy units.
- Details the manual leaves unspecified: Conquests-specific mission cost equations, success probabilities, incident response thresholds, precise per-mission technology gates, and selected operation outcomes.
- Supplementary source ID(s): CIV3-CONQUESTS-CIVILOPEDIA; CIVFANATICS-ESPIONAGE-STUDY; CIVWIKI-CIV3-ESPIONAGE
- Related manual rules: None

### Native unit capability cutover (2026-11-16)

- Source ID: CIV3-UNIT-PRTO-LAYOUT
- Title / author: `QueryCiv3/BiqSections/Prto.cs`, OpenCiv3 contributors
- URL: <https://github.com/C7-Game/OpenCiv3/blob/main/QueryCiv3/BiqSections/Prto.cs>
- Source type: Supplementary reverse-engineered BIQ field layout, not official gameplay documentation
- Scope: Conquests PRTO record layout; checked 2026-11-16.
- Claims supported: Flags1 maps Radar [0].5, all terrain as roads [0].4,
  carry aircraft [1].0, carry foot units only [1].6, tactical missiles [2].7,
  carry tactical missiles [3].0, and hidden nationality [2].1. Flags3 maps
  Load/Unload/Airlift/Pillage/Airdrop to [4].0–5 and Intercept to [12].2.
  `Capacity`, `OperationalRange`, `RequireSupport`, and `ZoneOfControl` are
  separate fields. The checked-in BIQ extraction supplies their values.
- Limitations: These bit names are reverse engineering, not evidence that
  the retained generic engine's algorithms match the executable.

- Source ID: CIV3-EDITOR-UNIT-ABILITIES
- Title / author: “Editor Rules - Explained”, Bluemofia, CivFanatics forums,
  posted 2007-03-02, edited 2016-11-20.
- URL: <https://forums.civfanatics.com/threads/editor-rules-explained.210035/>
- Source type: Supplementary community explanation; checked 2026-11-16.
- Scope / section: Civilization III editor, Units / unit abilities.
- Claims supported: Radar sees two squares in every direction; all terrain as
  roads changes terrain movement; transport-only-foot restricts passengers.
- Limitations: Includes cracked-editor options and community corrections.
  Ordinary ground sight (one square), treasury-only support, aircraft
  missions, and naval cargo are independently described by the official manual.

Native runtime expectations:

- Manual printed pp. 38–39: national support is paid in gold; no food or
  shields, and home-city bookkeeping does not determine eligibility. Use the
  native Conquests government tables and PRTO `RequireSupport` values.
- Manual printed pp. 73, 77–78: aircraft use operational range for missions;
  the generic planner's one-turn budget is engine state, not a new editable
  fuel statistic. Aircraft rebase/path planning remains an adaptation; exact
  Conquests mission-only movement is not established by this cutover.
- Manual printed pp. 77–78: ground observation is normally one tile;
  naval transports carry ground units, carriers carry aircraft, and nuclear
  submarines carry Tactical Nukes only. PRTO capacity is authoritative.
- PRIMA-CIV3-GUIDE p. 174: experience changes HP, not attack/defense power.
  The generic veterancy view uses native experience levels with neutral
  power and movement modifiers; old serialized compatibility fields remain.
- Native PRTO capabilities replace the removed foreign-unit crosswalk. No
  inherited per-target Destroyer/Aegis multipliers, leader GameLoss/Unique
  flags, Explorer/Partisan ZOC exemptions, or percentage class attrition are
  supplied as native unit statistics.
- Remaining algorithm gaps: the generic action/effect graph, ZOC blocking,
  aircraft flight/refueling planner, recovery formulas, and conquest partisan
  extension are retained implementation behavior, not claims of Civ III
  parity. Barbarian/partisan unit IDs are explicit native AI configuration;
  they do not grant additional PRTO abilities. War weariness has not been
  implemented here; the removed per-home-city military-unrest charge is not
  used as a substitute.
