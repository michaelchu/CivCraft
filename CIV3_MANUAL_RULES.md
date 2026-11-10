# Civilization III manual rules catalog

Status: **Terrain entries started.** The entries below capture the water and
special land terrain facts used in the current map model. A broader
manual-to-code audit remains outstanding.

## Authority and interpretation

- Use the supplied manual as the authority for the intended Civilization III
  rule wherever it states the rule.
- Record the manual page for every rule. Include a short quotation only when
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
- Do not treat Freeciv behavior, implementation details, or existing tests as
  Civ III evidence. They may be noted separately as implementation history,
  never as the expected result for a manual-based rule.

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
- URL or local file: [`civ3_manual.pdf`](civ3_manual.pdf), project root
- Source type: Original game material
- Edition / expansion / patch scope: Base Civilization III manual, 2001
- Pages, table, figure, or section: p. 80, Hit Points and Damage; p. 83,
  Bombardment; pp. 206–207,
  terrain yield and resource tables
- Retrieved or checked date: 2026-11-09
- Claims this source supports: Coast, Sea, and Ocean base yields and resource
  habitats; Flood Plains and shield yielding Grassland yields; new units
  generally have 3 hit points, Veterans 4, and Elites 5 (p. 80)
- Limitations or conflicts: Does not specify procedural map-generation
  thresholds, the frequency of shielded Grassland, or Conquests-only Volcano
  and Marsh rules. The manual says "generally" for starting hit points and
  does not enumerate Conquests unit-specific hit-point bonuses.

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
- Pages, table, figure, or section: printed pp. 108–110, “Those Wonderful Workers” and “The Workers you capture”
- Retrieved or checked date: 2026-11-10
- Claims this source supports: captured foreign Workers complete jobs at half the rate of the owner's native Workers; stacking Workers combines their progress rates.
- Limitations or conflicts: This is a PTW-era guide, not a Conquests manual. Conquests PRTO `WorkerStrength` remains authoritative for the editable source value, and the installed campaign value (1.0 for Worker) is interpreted against the native worker progress baseline of 2.

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
- URL or local file: [online manual](https://www.scribd.com/document/764777075/Sid-Meier-s-Civilization-III-Conquests-Manual)
- Source type: Original game material
- Edition / expansion / patch scope: Civilization III: Conquests
- Pages, table, figure, or section: pp. 38–39, “Clear Damage” worker action
- Retrieved or checked date: 2026-11-10
- Claims this source supports: Bombardment can cause craters; Workers clear them with the Clear Damage command.
- Limitations or conflicts: The manual does not specify which unit field enables craters, the exact creation probability, or the yield penalty.

- Source ID: CIV3-CONQUESTS-CIVILOPEDIA
- Title / publisher: *Conquests* installed Civilopedia text, Firaxis Games
- URL or local file: [`../source/Conquests/Text/Civilopedia.txt`](../source/Conquests/Text/Civilopedia.txt), “Craters” entry
- Source type: Original game material
- Edition / expansion / patch scope: Civilization III: Conquests
- Pages, table, figure, or section: “Craters,” lines 311–320
- Retrieved or checked date: 2026-11-10
- Claims this source supports: Craters can form in non-city squares when bombardment eliminates the last unit or terrain improvement; they reduce food, production, and commerce by one, and Workers can clear them.
- Limitations or conflicts: “May be formed” does not state an exact random chance. The installed Civilopedia describes the effect but not its PRTO flag mapping; the installed BIQ supplies `CreateCraters` values.

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
- Implementation: editable unit records now feed the Bombard action, range
  legality, strength roll, rate of fire, stack target selection, and lethal
  land/sea behavior. Existing adjacent bombard execution reads the same source
  strength and lethal flags.
- Unresolved: city improvements, population, terrain improvements, fortress
  damage, target priority, bombard defense, and automatic bombard orders are
  not implemented. Human air missions using operational range are also not
  wired to this target action. The UI does not yet draw the full red target
  grid.

### Bombard visual effects and craters

- Expected rule: PRTO `BombardEffects` enables an explosion effect on bombard;
  `CreateCraters` allows bombardment damage on a non-city tile when the last
  unit or terrain improvement there is eliminated. Craters reduce food,
  production, and commerce by one and Workers can clear them.
- Source: Conquests BIQ PRTO `BombardEffects` and `CreateCraters`; CIV3-CONQUESTS-MANUAL,
  pp. 38–39; CIV3-CONQUESTS-CIVILOPEDIA, “Craters”; CivFanatics “Editor Rules - Explained,”
  “Bombard Fx” and “Create Craters” entries.
- Implementation: `BombardEffects` drives a target explosion overlay through
  the existing combat event. `CreateCraters` persists a tile marker after a
  flagged bombard eliminates its final unit on a non-city tile with no
  remaining modeled terrain improvement. The marker subtracts one from each
  tile yield and is clearable by Workers through the existing damage-cleaning
  action. UI rendering uses the existing crater atlas.
- Unresolved: source text does not state the crater probability; current
  implementation treats an eligible event as certain. Bombarding and removing
  terrain improvements is not implemented, so crater creation from the final
  improvement being destroyed is deferred. The visual effect is a native
  overlay, not playback of the original unit-specific bombard animation.

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
