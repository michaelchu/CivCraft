# CivCraft gameplay and parity audit

This guide describes current CivCraft behavior and how thoroughly it has been compared with Freeciv. It is not a claim of complete Freeciv parity. The reference checkout is `/Users/michaelchu/Documents/projects/civjs/reference/freeciv`; the gameplay ruleset is `data/civ2civ3/`. Detailed numeric comparisons, native traces, known gaps, and focused vectors are recorded in [MECHANICS.md](MECHANICS.md).

## How to read parity status

**Verified** means a specific mechanic has been compared with reference ruleset or source evidence. It does not mean the whole system is equivalent. **Partial** means there is meaningful source-based implementation or testing, but important paths remain unaudited or deliberately different. **Not audited** means this guide makes no parity assertion. Tests cover selected cases; they do not prove complete-match equivalence, especially where random draws and phase timing matter.

### World and map

| Aspect | Status | Audit result |
|---|---|---|
| Setup, world and terrain/resource generation | **Parity verified for supported Fractal modes** | Civ2Civ3 Fractal generation is compared against native Freeciv with ten goldens: five seeds each for WrapXY and WrapX, covering dimensions, terrain, elevation, extras/resources, and start positions. WrapXY retains reference polar terrain. WrapX intentionally converts the polar rows to ocean and clears resources, rivers, and villages there. |
| Terrain rendering and resource artwork | **Amplio2 parity verified for audited paths** | Sprite selection, terrain blending, map layer ordering, resources and improvements follow Amplio2 reference assets/specifications; targeted tests and visually inspected captures are documented in RENDERING_PARITY.md. Coverage does not include every possible map or rendering state. |
| Terrain work and improvement behavior | **Parity verified for documented core rules; edge cases partial** | Ruleset requirements, terrain eligibility, key technology gates, conflicts, work completion, and output effects have source comparisons and focused tests for core improvements. Rare terrain/unit combinations, all task lifecycle edges, and exhaustive coverage of every valid/invalid combination are not proven. |
| Strategic resource supply | Absent in both current CivCraft and reference | Resources give local tile-yield bonuses; no stockpile or connected-resource unit gate exists. Adding such gates would be a gameplay design deviation. |

### Cities, research, and government

| Aspect | Status | Audit result |
|---|---|---|
| City founding, growth, citizens | **Parity verified for tested core rules; audit incomplete** | Founding eligibility/spacing, growth/famine resolution, population costs/loss, worker/specialist reassignment, and selected citizen-manager behavior have reference comparisons and focused scenarios, including native-oracle checks. A full-turn test growing a city from size 1 past 16 is still missing; every city effect and rare edge case is not certified. |
| Production and buildings | Partial | Shared legality checks and selected unit/building gates, costs, switching, and completion behavior are source checked. The whole building/effect matrix and every production edge case are not audited. |
| Research and technology | Partial | Catalog, prerequisites, unlocks, and selected cost arithmetic/advisor paths have evidence, including compiled C vectors. Full phase timing and every technology effect are not proven equivalent. |
| Government and economy | Partial | Government data, selected output/upkeep effects, tax priorities, and citizen allocation have detailed comparisons. Whole-economy equivalence for all effects/states is not established. |

### Units and military

| Aspect | Status | Audit result |
|---|---|---|
| Units and requirements | Catalog verified; behavior partial | All 57 unit types are catalogued. Stats fixtures, flags, roles, tech/government requirements, and upgrade/obsolescence chains have substantial reference evidence. Special-unit behavior and all action combinations remain incomplete. |
| Movement, vision, transport, air | Partial/strong for specified paths | Map distances, movement costs, and several transport/air/refueling paths are checked. Exhaustive action and phase-order parity is not established. |
| Combat, healing, veterans | Partial/strong for core formulas | Combat rounds, power, odds, defender choice, healing, and selected promotion rules have source comparisons. Complete random-draw or campaign equivalence is explicitly unproven. |

### Economy, diplomacy, and end game

| Aspect | Status | Audit result |
|---|---|---|
| Trade, diplomacy, espionage | Partial | Many mechanics have dedicated implementations and tests; every threshold, probability, diplomatic consequence, and phase interaction has not been audited. |
| AI | Partial; not equivalent | Several advisors use source algorithms and C-oracle vectors. MECHANICS.md lists known differences in policy consumers, caches, task timing, list ordering, special behavior, and random draws. |
| Victory and score | Partial/scope-specific | Spaceship construction and launch are implemented with scenario coverage. Conquest is a focused-build condition; full parity for native end conditions, score, timeout, and race edge cases is not established. |

### Interface

| Aspect | Status | Audit result |
|---|---|---|
| UI | Separate parity scope | CivCraft has its own interface and controls. UI review is documented in UI_REVIEW.md and UI_PARITY_AUDIT.md, not treated as reference-client parity here. |

## Match and turn flow — setup verified, full loop partial

CivCraft is a turn-based local single-player game. You lead Rome against five AI civilizations on a generated world. You explore, found and develop cities, select research and production, manage units and diplomacy, then end the turn. Turn resolution advances city activity, research, upkeep, unit recovery, AI orders, and other scheduled systems. Exact whole-turn equivalence is not established; MECHANICS.md records specific phase-order checks and unresolved timing differences.

The match uses Civ2Civ3 setup values with a generated map. Reference topology and map dimensions are checked for the supported wrap modes. The default cylinder (WrapX) is an intentional CivCraft mode; the source Civ2Civ3 preset uses a torus (WrapXY). Wrap setting affects generation, movement, city ranges, visibility, pathfinding, and rendering. Source setup values and numeric details are in MECHANICS.md under Match and Configurable wrapping.

Opening units are Settlers, Worker, Explorer, and Diplomat. Starting technology follows the configured one-free-advance setup. The exact randomized assignment/startup draw sequence is not fully native-equivalent; see the random-state notes in MECHANICS.md.

## World, terrain, resources, and improvements — partial parity

Terrain sets base food, shields (production), trade, movement cost, city eligibility, and defensive modifiers. Rivers and improvements modify output or movement. Terrain values listed in MECHANICS.md are tied to the matching Civ2Civ3 ruleset sections. Generator output has native goldens for terrain, elevation, extras, and start positions under stated seeds and wrap settings; this does not prove all map seeds or every optional setup path.

Resources such as Wheat, Iron, Coal, and Oil are map extras. In the reference, resource definitions specify local food, shield, and trade bonuses. CivCraft applies these bonuses to tile yield when the city uses the tile. They are not quantities collected into a national inventory. Oil has additional terrain-improvement uses: Oil Wells or Oil Platforms can be built under their technology/terrain rules and affect output. They do not create a unit-building stockpile.

Workers and Settlers can perform supported terrain work, including roads, irrigation and mines. Roads and railroads affect movement and can affect tile output. The documented task times, terrain rules, and some effects have direct comparisons. Not every work action, cancellation, conflict, promotion draw, or AI worker scheduling edge case has been audited as a complete system.

Tile ownership and city work are separate concepts. CivCraft tracks tile owners for territorial/border-related behavior; a city’s `worked_tiles` determine which tiles contribute output. Working a resource tile is not a resource-ownership or supply rule. There is currently no strategic-resource supply network. A future framework could require that a resource tile be owned, optionally have an extractor/improvement, and connect by continuous roads/railroads to an owned city. This is proposed gameplay, not present implementation or reference parity.

## Cities, citizens, and production — core founding/growth rules parity checked; audit incomplete

Settlers found cities on legal land, subject to city spacing. Founding eligibility and distance, growth/famine resolution, population loss, and citizen management have reference comparisons and focused gameplay scenarios; the city manager also has native-oracle checks. These support parity claims for those specific rules and tested flows, not completion of the full city audit. The high-population growth test currently exercises the growth resolver with a steady surplus rather than full turn processing; a city may stop growing in the full simulation when disorder removes its food surplus. Cities grow when food fills the growth box and lose population under famine; size-one cities can be destroyed by famine. Population supplies workers, who can work local tiles or become specialists. Every effect and rare city-state edge case remains outside this verified subset.

City output is generated from the center and worked tiles, then modified by supported buildings, government, and effects. Food supports growth and units; shields complete production and may pay support; trade is divided into science, luxury, and tax according to rates. The system includes many source-based effects, but there is no claim that every native effect interaction, citizen-feeling condition, celebration, disorder case, or integer boundary has been exhaustively audited.

Cities produce units, buildings, and supported special targets. The simulation validates production through shared legality checks before accepting a change and when resolving completion. Unit production can cost population and units can require food/shields/gold upkeep. Production switching, bought production, support, and effects have focused checks. CivCraft intentionally changes the Barracks progression: a city builds Barracks once, and its veteran training remains active as the owner researches Gunpowder and Mobile Warfare. The training effect stays the same; the technologies do not provide additional Barracks ranks. Barracks II and Barracks III are retained as legacy catalog entries but are not separately buildable; their effect upgrades automatically. This differs from the reference ruleset's separately constructed, technology-gated tiers. The complete building catalog and every cost/obsolescence/availability edge case are not certified by this guide.

## Research and government — partial parity

The research tree contains 87 named advances. Players choose a goal; the game follows missing prerequisites and applies accumulated science to the current advance. Advances unlock units, buildings, governments, and work abilities. Philosophy grants a free advance to the first discoverer, and Future Tech repeats after the named tree. Catalog/prerequisite data, unlock listing, and selected research-cost and AI-want arithmetic have reference comparisons and vectors. Full cost equivalence for every game state, all effect interactions, and all phase timing is unproven.

Governments change economic rates, unit support, and other effects. Government access may require an advance; adoption can involve revolution/anarchy. Government data and selected output/upkeep, tax, AI evaluation, and citizen rearrangement paths have detailed source checks. The effect engine supports known Civ2Civ3 effects rather than acting as a fully generic ruleset interpreter; remaining advisor/cache issues are listed in MECHANICS.md.

## Units, movement, combat, and recovery — catalog checked, behavior partial

The unit catalog has 57 types spanning civilian, land, naval, air, missile, and special roles. Unit ordering, stats fixtures, metadata, roles, many flags, requirements, and upgrade/obsolescence chains are checked against the reference. Ordinary production checks technology, government, no-build status, successor availability, and city/naval access. Fanatics require Guerilla Warfare and Fundamentalism. Nuclear requires Nuclear Fission and the Manhattan Project (which any player may build). Strategic-resource prerequisites are defined by neither the current reference nor current CivCraft.

Movement uses six map directions, terrain costs, roads, unit classes/flags, remaining movement, and wrapping. Distance and base movement formulas have direct native checks. CivCraft implements worker actions, exploration, fortification, trade, cargo, diplomacy/espionage actions, paradrop, bombardment, and air operations. Selected transport and aircraft paths are specifically audited, including refueling and return paths; the full action matrix and phase timing are not established. Leader loss behavior, Storm initialization/random movement, and some dual-role scheduling are known areas needing dedicated audits.

Combat resolves rounds using attack/defense power, hit points, firepower, and applicable modifiers. Terrain, cities, fortification, veteran status, and remaining movement affect combat. Core round arithmetic, odds, defender selection, power stages, recovery, and selected veteran behavior have detailed comparisons and tests. These checks do not prove that complete campaign outcomes or random number consumption match native Freeciv. ZOC, capture, bombardment, nuclear, amphibious, and special-unit mechanics are implemented in specific modules; consult those code paths/tests for individual edge cases rather than assuming blanket parity.

## Economy, trade, diplomacy, and espionage — partial parity

The economy uses city output, tax/science/luxury allocation, gold reserves, rush purchases, and unit/building upkeep. Resource tile output is local. Tax management has source-derived behavior and compiled C vector coverage; selected government effects and costs have comparisons. A full audit of every building, effect, upkeep corner case, and economic decision over an entire campaign remains outstanding.

Caravan/Freight units and trade routes provide trade benefits. Trade has dedicated simulation code and scenario tests, but this guide does not certify every native route valuation, path, cancellation, or city-pair edge case.

Diplomacy includes contact, embassies, treaties/pacts, shared vision/information, technology exchange, and war/peace changes. The code has source-based handling for contact expiry, access, intelligence snapshots, obligations, and selected AI behavior. Negotiation thresholds, all clauses, random outcomes, and every phase interaction are not exhaustively compared.

Diplomats and spies can perform operations on rival cities. Espionage, investigation, sabotage/theft, bribery, and related consequences have their own implementation and tests. Exact odds/costs, detection, diplomatic side effects, and every target-state combination are not certified as fully native-equivalent by this guide.

## AI — partial parity, known differences

Five AI civilizations select governments, taxes, research, city production, worker tasks, movement, military actions, diplomacy, and victory goals. Multiple advisor paths preserve native scoring, ordering, and arithmetic; some are checked against literal C vectors. This is component-level evidence, not proof of identical decisions.

MECHANICS.md records known gaps, including some threat/knowledge-policy consumers, technology-effect valuation, advisor cache timing, historical fog extras, task/activity lifecycle details, incomplete native list consumers, dedicated hunter production, startup/random sequencing, and complete-match random-draw equivalence. AI behavior can therefore differ even where the underlying action is legal and the specific scoring component is source-derived.

## Victory and ending — partial/scope-specific

The game supports conquest and spaceship victory. CivCraft’s focused conquest condition is defined around eliminating rivals’ cities and city founders; MECHANICS.md explicitly identifies this as a focused-build scope choice. Do not treat it as proof that all native conquest settings and elimination edge cases match.

The spaceship system tracks parts, applies build/placement limits, assembles and launches ships, and evaluates arrival/race status. Relevant code is in `src/sim/spaceship.rs`, `src/sim/buildings.rs`, and diplomacy logic; selected scenarios exist. A full comparison of all native destruction, delay, warning, arrival, score, timeout, and simultaneous-end cases is beyond the verified evidence summarized here. Score/achievement calculations are separate and should not be read as parity-verified victory logic.

## Interface and controls — separate audit

CivCraft uses its own interface. Controls, saves, seeds, and wrap options are described in [README.md](README.md). Visual and interaction review is in `UI_REVIEW.md`; the reference-client comparison is in `UI_PARITY_AUDIT.md`. UI resemblance or a passing UI scenario does not prove simulation-rule parity.

## Source map for continued audit

- Ruleset source: `/Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/civ2civ3/`.
- Map/topology/generation: `src/mapgen.rs`, `src/sim/topology.rs`; details and goldens in MECHANICS.md.
- Tile/resource data and yields: `src/sim/model.rs`, `src/sim/cities.rs`, `src/sim/output_effects.rs`.
- Unit catalog and availability: `src/sim/unit_catalog.rs`, `src/sim/mod.rs`, `src/sim/buildings.rs`.
- Cities/citizens/production: `src/sim/cities.rs`, `src/sim/citizens.rs`, `src/sim/buildings.rs`.
- Research/government/economy: `src/sim/research.rs`, `src/sim/research_phase.rs`, `src/sim/government.rs`, `src/sim/economy.rs`, `src/sim/upkeep.rs`.
- Terrain work/movement/combat/air: `src/sim/workers.rs`, `src/sim/terrain_work.rs`, `src/sim/movement.rs`, `src/sim/offense.rs`, `src/sim/aviation.rs`.
- Diplomacy/trade/espionage/victory: `src/sim/diplomacy.rs`, `src/sim/trade.rs`, `src/sim/espionage.rs`, `src/sim/spaceship.rs`, `src/sim/score.rs`.
- Existing fine-grained evidence and remaining known gaps: [MECHANICS.md](MECHANICS.md).
