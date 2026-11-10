# Civ III diplomacy and espionage mission design

## Goal and source boundary

Replace the inherited Diplomat/Spy unit paths with Civ III-style actions initiated from a civilization or city context. Diplomatic agents are never trained, spawned, moved, transported, captured, upgraded, or supported as map units. The persistent game state is a diplomatic relationship plus optional embassy and spy assets for each player pair; actions execute through the diplomacy/espionage interface.

Primary authority is the repository manual, `civ3_manual.pdf`, Chapter 13 “Diplomacy and Trade” (printed pp. 137–149), plus the unit-system note in Chapter 8 (printed p. 38). The installed Conquests supplement is `../source/Conquests/Text/Civilopedia.txt`; where it revises or adds rules, prefer Conquests text for the target ruleset. Manual statements not repeated or contradicted by Conquests text are identified as manual-backed expectations, not assumed expansion changes.

## Confirmed Civ III behavior

### Diplomacy and embassies

- Contact must exist before embassy actions. Writing enables establishing an embassy. The Conquests Civilopedia says the Espionage screen itself is available after Writing; some actions become available at later technologies.
- Establishing an embassy costs gold and creates one diplomatic asset in the rival’s capital. The screen lists contacted civilizations for which the player is not at war and does not already have an embassy. It is established once per rival; war closes it, and peace reopens it.
- The embassy/diplomatic screen can investigate a rival city and attempt to steal a technology. The first capital investigation on embassy establishment is described as having no failure chance or incident. The manual does not make clear whether that first look consumes an additional mission fee.
- Diplomatic missions cost gold. The Conquests Civilopedia’s Espionage screen has an opponent selector, mission class and type, optional city selector, operational-cost/risk selector, and execute/cancel controls. The user can cancel before execution.
- Diplomatic agreements such as Right of Passage and Military Alliances require an embassy and a friendly relationship; Nationalism is required for Mutual Protection Pacts and Trade Embargoes. These are diplomacy-screen agreements rather than unit actions.

### Espionage assets and actions

- The manual says an Intelligence Agency plus Espionage technology enables planting a Spy through an embassy. A failed attempt can create an international incident. Conquests Civilopedia likewise lists the Intelligence Agency as enabling espionage and says Spy missions become available after Espionage; the Conquests Espionage screen itself is available from Writing.
- A planted Spy is a persistent asset/status associated with a rival, not a trainable or movable unit. Conquests UI text describes embassy and spy icons by opponent, tracks Spy experience, and offers actions from the screen. A Spy preserves information about enemy unit locations during war even while the embassy is closed.
- Conquests Civilopedia lists these mission types: Establish Embassy; Plant Spy; Steal Technology; Steal Military Plans; Initiate Propaganda; Expose Enemy Spy; Investigate City; Steal World Map; Sabotage Production.
- The manual says Sabotage destroys half the shields accumulated in the current project. Conquests Civilopedia supersedes this: Sabotage Production destroys all shields in the city’s current production box.
- Steal Military Plans reveals the rival’s military-unit locations. Propaganda can cause the target city to revolt and defect. Conquests Civilopedia says propaganda targets the city associated with the Spy. The manual says Democracy is immune, Capitals and Courthouses lower success, and civil disorder makes a city more vulnerable; verify these legacy details against Conquests runtime before implementing them as rules.
- Expose Enemy Spy is counterespionage. The manual says it requires a successfully planted Spy in the rival capital; success removes the rival operative. Conquests Civilopedia describes attempting to expose an infiltrator but does not specify the exact target/location requirement.
- Mission success and discovery are separate outcomes: the manual explicitly says an action can succeed and still cause an incident. Discovery causes an international incident and can provoke war; outcomes may depend on diplomatic attitude.

### Source conflicts / edition changes

- The base manual includes Plant Disease (poisoning a city to reduce population). Plant Disease is absent from the Conquests Civilopedia’s explicit mission list. Keep it out of the Conquests campaign mission catalog unless a Conquests-specific source confirms it.
- The base manual says Spy missions remain available during war when an operative is planted; Conquests Civilopedia’s espionage-status UI and the manual support this distinction between a closed embassy and a planted Spy.
- Diplomat and Spy are not unit identities in Civ III. The manual explicitly says they no longer move around the map and missions run through embassies. Any `UnitKind::Diplomat`/`UnitKind::Spy` support inherited from Freeciv must be removed from campaign production, movement, transport, capture, upgrade, upkeep, AI, and unit UI.

## Proposed screen and API boundary

Use the rival civilization as the top-level context, with a city target only for missions that require one. The map’s capital/city icons may open this screen, but should not create an agent entity on the map.

Suggested rule-facing types:

```rust
struct EspionageAssets {
    embassy: EmbassyState,
    spy: Option<SpyAsset>,
}

struct SpyAsset {
    // Persistent source-backed status, not a Unit or map coordinate.
    experience: SpyExperience,
}

enum DiplomaticMission {
    EstablishEmbassy,
    InvestigateCity { city: CityId },
    StealTechnology { technology: Option<TechId> },
    PlantSpy,
    SabotageProduction { city: CityId },
    Propaganda { city: CityId },
    StealMilitaryPlans,
    StealWorldMap,
    ExposeEnemySpy,
}
```

Keep `SpyAsset` fields minimal until the exact source model for spy location/experience is verified. Do not add a unit id, tile, movement points, home city, or transport association.

Expose separate pure eligibility/quote operations and an effectful execution operation:

- `available_missions(actor, rival) -> Vec<MissionOption>`: checks contact, Writing, Espionage, Intelligence Agency, embassy/spy state, war/peace state, target city, target immunity, and any source-backed diplomatic agreement prerequisites.
- `quote_mission(actor, rival, mission, risk_level) -> MissionQuote`: displays gold cost and risk tier before the player commits. Preserve the Conquests screen’s ability to cancel without consuming gold or changing state.
- `execute_mission(actor, rival, mission, risk_level) -> MissionResult`: charge quoted gold, resolve success independently from discovery/incident, apply the mission effect, update embassy/spy status and experience, and record diplomacy incidents/events.

Store the underlying embassy/spy asset per ordered `(owner, rival)` relationship, alongside the existing diplomacy state. Persist it in the current campaign save schema. Reopening the screen must derive available options from current tech/buildings, relationship status, target-city state, and assets rather than maintaining UI-only mission availability.

## Required game-state checks

- Writing: opens the screen and enables embassy establishment; early diplomatic missions are available subject to their own gates.
- Contact known: required to target a civilization.
- At war: ordinary embassy establishment is unavailable. Existing embassy information shuts off during war. A planted Spy keeps its espionage intelligence/actions active.
- Espionage technology and Intelligence Agency: required for covert spy actions according to the manual and Conquests technology/building entries.
- Embassy asset: required for regular diplomatic mission access and planting a Spy through that embassy.
- Planted Spy asset: required for covert actions, counterespionage and spy-based military-location reporting.
- City target: required only for city missions. Validate the city belongs to the selected rival and is eligible (e.g., Democracy immunity to Propaganda per the manual; verify Conquests scope).
- Gold: embassy establishment and missions consume treasury gold. UI must quote a cost and reject/disable actions when the player cannot afford them.

## Details intentionally left unresolved

Do not invent a cost or probability formula. The manual and Conquests Civilopedia establish that costs and operational risk are selectable, but they do not give their exact numeric formulas. Before implementing exact numbers, inspect Conquests executable behavior or a primary, version-matched rules source for:

1. Embassy and mission price formulas, including map size, target strength, city size, difficulty, and spy experience effects.
2. Operational-cost options and their exact success/discovery probability curves; keep success and incident rolls separate.
3. Whether each Conquests mission requires an embassy, a planted Spy, both, or neither; the Civilopedia’s list is not fully explicit about per-mission prerequisites.
4. Whether a planted Spy is city-local or only civilization-local, how it changes location, and what “Spy experience” measures or persists.
5. Which tech level unlocks each mission, and whether an Intelligence Agency is mandatory for every covert action or only planting.
6. Propaganda immunity/chance modifiers in Conquests (Democracy, capital, Courthouse, city disorder), and Sabotage Production edge cases for wonders, completed builds, or zero shields.
7. Exact incident, reputation, attitude, and war-response effects for success, failure, discovery, and exposure.
8. Whether stealing the World Map/Plans exposes data immediately, until turn end, or for another duration in the Conquests build.
9. Whether the manual’s free initial capital investigation survives in Conquests, and whether it is a one-time free action or included in embassy setup.

## Evidence index

- `civ3_manual.pdf`: Chapter 8 “Units,” printed p. 38 (no Diplomat/Spy units); Chapter 13 “Diplomacy and Trade,” printed pp. 137–149 (embassies, diplomatic actions, Espionage, covert actions, incidents).
- `../source/Conquests/Text/Civilopedia.txt`: `GCON_Espionage` and `GCON_Espionage_Missions` (lines 817–928); `GCON_Embassies` (1607–1620); Intelligence Agency (5762–5765); Espionage technology (6535–6539).
- `../source/Text/Civilopedia.txt`: base-game Espionage and Embassies entries (lines 631–663 and 1306–1315), useful for historical comparison only.
- Primary online rendering of the manual: <https://usermanual.wiki/Document/Civilization3Manual.1841378190.pdf>.
