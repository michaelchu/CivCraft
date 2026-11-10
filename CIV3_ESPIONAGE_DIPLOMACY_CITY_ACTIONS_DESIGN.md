# Civ III espionage and diplomacy as city actions

## Purpose and scope

Replace the legacy trainable Diplomat and Spy unit paths with a Civ III style diplomacy and espionage interface. Missions are initiated from a diplomacy/espionage screen or city context; no agent is trained, moved on the map, or represented by a combat `UnitKind`. This note records source-backed behavior and a proposed API boundary. It does not define unverified formulas.

Conquests is the target ruleset. Where the base manual differs from the Conquests Civilopedia, use the Conquests description and flag the difference.

## Confirmed behavior

### Diplomatic access and embassies

- Writing unlocks the Espionage screen and permits embassy establishment. The Conquests Civilopedia says the screen is available after Writing and describes building an embassy in a rival capital by double-clicking its capital icon. (Conquests `Text/Civilopedia.txt`, `GCON_Espionage` lines 826-27 and `GCON_Embassies` lines 1611-18.)
- Embassies are per-player-pair diplomatic assets associated with the opponent's capital. Establishing an embassy costs gold; other diplomatic missions also cost gold. The source gives no numeric formula. (Conquests Civilopedia lines 1611-18.)
- An embassy enables diplomatic interactions such as Right of Passage and Military Alliances; Nationalism adds Mutual Protection Pacts and Trade Embargoes. (Conquests Civilopedia lines 1611-16.)
- The repository manual says embassies require contact and Writing, cannot be established while at war, close during war, and reopen after peace; an embassy can be established only once. It also describes investigating a city and stealing technology as diplomatic missions. (Civ III manual, ch. 13, “Embassies” and “Diplomatic Missions,” printed pp. 138-39.)
- The manual describes investigating the initial capital with no failure or incident. It does not clearly establish whether that initial investigation is free, so do not encode a free action without verifying it against Conquests behavior.

### Espionage availability and assets

- Conquests describes Espionage missions as requiring the Espionage technology and an Intelligence Agency. The Intelligence Agency Civilopedia entry says it allows espionage missions; Espionage tech says it permits building the Agency and conducting missions. (Conquests Civilopedia lines 821-23, 5762-65, 6535-39.)
- The Espionage screen tracks each opponent, embassy and planted-spy status, mission choices, target city when relevant, treasury/technology assets, spy experience, and operational-cost/risk choices. (Conquests Civilopedia lines 826-42.)
- A planted spy is a persistent diplomatic asset belonging to a player-opponent pair, not a map unit. The screen indicates whether one is planted. Missions that say “use a spy” or target the city where the spy resides depend on this asset. The exact location and movement semantics of a planted spy are not specified in these passages.
- The manual says a spy can remain planted during war and can reveal rival unit locations. A failed or discovered mission can cause a diplomatic incident; success and discovery are distinct outcomes. (Manual, ch. 13, “Espionage,” printed pp. 145-49.)

### Conquests mission catalog

The Conquests Civilopedia lists these actions (lines 847-71, 902-28):

| Action | Source-backed effect / gate |
| --- | --- |
| Establish Embassy | Attempts to establish embassy with selected civilization; Writing-era diplomatic action. |
| Plant Spy | Attempts to plant a spy with selected civilization. Requires Espionage-era capability per mission screen description. |
| Steal Technology | Attempts to take a technology from selected civilization. |
| Steal Military Plans | Reveals locations of all rival military units. |
| Initiate Propaganda | Targets the city where the spy resides; success causes revolt and defection. |
| Expose Enemy Spy | Attempts to expose an enemy spy in one of your cities. |
| Investigate City | Reveals information about a selected rival city. |
| Steal World Map | Attempts to take a rival world map. |
| Sabotage Production | Destroys all shields in the target city's current production box. |

The Conquests description says “most” missions unlock after Espionage and that spy missions unlock after Espionage, but does not give a complete per-mission technology matrix. Therefore, treat the table as a catalog, not a complete availability specification.

## Edition differences to honor

- The base manual says sabotage destroys half the shields in the current project; the Conquests Civilopedia explicitly says it destroys all shields in the production box. Implement the Conquests behavior.
- The base manual includes Plant Disease as a spy mission. The Conquests mission catalog does not include it. Exclude Plant Disease from the initial Conquests roster unless a Conquests rules source establishes otherwise.
- The manual describes Propaganda as city defection, with Democracy immune and capitals/Courthouses lowering success; Conquests Civilopedia confirms defection but does not repeat those modifiers. Verify these conditions in Conquests game data/source before implementing them as rules.

## Suggested UI and API boundary

Keep mission execution in a diplomacy/espionage service and represent persistent assets separately from units:

```text
DiplomacyState
  relations[player][opponent].embassy: EmbassyState
  relations[player][opponent].spy: SpyAssetState

DiplomacyMission
  EstablishEmbassy | PlantSpy | StealTechnology | StealMilitaryPlans
  | InitiatePropaganda | ExposeEnemySpy | InvestigateCity
  | StealWorldMap | SabotageProduction

available_missions(player, opponent, game_state) -> MissionAvailability[]
quote_mission(player, opponent, mission, target?, risk_setting?) -> MissionQuote
execute_mission(player, opponent, mission, target?, risk_setting?) -> MissionResult
```

`MissionAvailability` should include a source-backed unavailable reason (missing Writing, missing Espionage/Agency, no contact, war restriction, absent planted spy, invalid target, etc.). `MissionQuote` should carry a gold cost and risk choice only after their formulas are recovered. `MissionResult` should distinguish mission success, spy discovery, diplomatic incident, and resulting state change; the manual makes clear that success and discovery are independent outcomes.

The UI should expose opponent selection, embassy/spy status, mission selection, a city selector when applicable, cost/risk controls, and execute/cancel actions, matching the Conquests screen flow (Conquests Civilopedia lines 826-42 and 881-99). Do not add recruit/train/move-agent controls.

## Facts still unresolved

Do not invent these values or gates during the first implementation pass:

1. Gold cost formulas for embassy establishment and each mission; how operational-cost/risk options alter price and detection chance.
2. Success, failure, discovery, incident, and war-trigger probability formulas, including modifiers from government, difficulty, target city, and spy experience.
3. Whether a planted spy is attached to a specific city, can change cities, or merely enables actions against a civilization. Conquests says propaganda targets the city where the spy resides, while other listed actions refer to civilization or selected city.
4. Exact tech/building gates for each mission and whether existing embassies are prerequisites for any particular action.
5. The precise “spy experience” state and how it changes mission outcomes.
6. Exact investigation payload, map/technology theft selection and duplicate-tech behavior, and military-plan intel duration.
7. Whether first-capital investigation is free in Conquests. The manual confirms no failure/incident for the initial capital investigation but is ambiguous on extra gold cost.
8. Propaganda immunity/modifier details in Conquests, and whether the base-manual Democracy, capital, Courthouse, and civil-disorder modifiers carry over.
9. Whether embassy closure/reopening at war, one-time establishment, and “contact required” are precisely Conquests rules. The manual confirms these; the Civilopedia embassy summary does not restate them.

Resolve these against the Conquests BIQ and executable behavior or a focused in-game reference before treating them as canonical rules. Keep unresolved values data-driven so later corrections do not require rewriting the mission flow.

## Evidence consulted

- Repository manual: `app/civ3_manual.pdf`, Chapter 8 (printed p. 38) and Chapter 13 (printed pp. 138-49). Accessible copy used for page/text cross-check: [Civilization III Manual](https://usermanual.wiki/Document/Civilization3Manual.1841378190.pdf).
- Conquests Civilopedia: `source/Conquests/Text/Civilopedia.txt`, entries `GCON_Espionage` / `DESC_GCON_Espionage` (lines 817-71), `GCON_Espionage_Missions` (lines 876-28), `GCON_Embassies` (lines 1607-20), `BLDG_Intelligence_Center` (around line 5762), and `TECH_Espionage` (around line 6535).
- Base-game Civilopedia: `source/Text/Civilopedia.txt`, `GCON_Espionage` (lines 631-63) and `GCON_Embassies` (lines 1302-15), used only to identify edition differences.
