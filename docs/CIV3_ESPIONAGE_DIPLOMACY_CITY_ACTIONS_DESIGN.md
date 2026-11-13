# Civ III espionage and diplomacy as city actions

## Purpose and scope

This is a research and design note for future espionage work, separate from P10. The user deferred espionage gameplay and chose not to add Diplomat or Spy units. The unit-free screen model described below is a candidate only; its actor mapping and locality are not selected. No implementation should be inferred from this note.

Do not use the Conquests espionage catalog as the target action catalog. Keep edition differences as compatibility evidence and label claims by source.

## Civ III interface compatibility evidence

### Diplomatic access and embassies

- Writing unlocks the Espionage screen and permits embassy establishment. The Conquests Civilopedia says the screen is available after Writing and describes building an embassy in a rival capital by double-clicking its capital icon. (Conquests `Text/Civilopedia.txt`, `GCON_Espionage` lines 826-27 and `GCON_Embassies` lines 1611-18.)
- Embassies are per-player-pair diplomatic assets associated with the opponent's capital. Establishing an embassy costs gold; other diplomatic missions also cost gold. The source gives no numeric formula. (Conquests Civilopedia lines 1611-18.)
- An embassy enables diplomatic interactions such as Right of Passage and Military Alliances; Nationalism adds Mutual Protection Pacts and Trade Embargoes. (Conquests Civilopedia lines 1611-16.)
- The Espionage screen lists contacted civilizations for which the player is not at war and does not already have an embassy. The manual says an embassy can be established only once; war closes it and peace reopens it. These details are manual-backed and should be confirmed for Conquests before being treated as implementation rules.
- The repository manual says embassies require contact and Writing, cannot be established while at war, close during war, and reopen after peace; an embassy can be established only once. It also describes investigating a city and stealing technology as diplomatic missions. (Civ III manual, ch. 13, “Embassies” and “Diplomatic Missions,” printed pp. 138-39.)
- The manual describes investigating the initial capital with no failure or incident. It does not clearly establish whether that initial investigation is free, so do not encode a free action without verifying it against Conquests behavior.

### Espionage availability and assets

- Conquests describes Espionage missions as requiring the Espionage technology and an Intelligence Agency. The Intelligence Agency Civilopedia entry says it allows espionage missions; Espionage tech says it permits building the Agency and conducting missions. (Conquests Civilopedia lines 821-23, 5762-65, 6535-39.)
- The Espionage screen tracks each opponent, embassy and planted-spy status, mission choices, target city when relevant, treasury/technology assets, spy experience, and operational-cost/risk choices. (Conquests Civilopedia lines 826-42.)
- A planted spy is a persistent diplomatic asset belonging to a player-opponent pair, not a map unit. The screen indicates whether one is planted. Missions that say “use a spy” or target the city where the spy resides depend on this asset. The exact location and movement semantics of a planted spy are not specified in these passages.
- The manual says a spy can remain planted during war and can reveal rival unit locations. A failed or discovered mission can cause a diplomatic incident; success and discovery are distinct outcomes. Discovery can provoke war, and diplomatic attitude may affect the response. (Manual, ch. 13, “Espionage,” printed pp. 145-49.)
- Expose Enemy Spy is counterespionage. The manual says it requires a successfully planted Spy in the rival capital and success removes the rival operative, while Conquests text does not specify that exact location requirement.

### Conquests mission catalog as UI reference

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

This table describes the Conquests interface, not P10 gameplay scope. The in-scope action set comes from C2C3 `actions.ruleset`, `effects.ruleset`, and the pinned Freeciv handlers. Map actions to the Civ III embassy/spy interface when possible. C2C3 actions without a direct Conquests mission analogue remain in scope and need an explicit unit-free adapter or a documented technical blocker; do not silently omit them or add Freeciv-only combat units.

A candidate unit-free interface could present diplomatic and covert missions through a diplomacy/espionage screen rather than through map units. It might support an opponent selector, mission class and type, an optional city selector, and execute/cancel controls. This is a proposal for later work, not an implementation decision.

## Edition differences relevant to the interface

- The base manual and Conquests Civilopedia describe sabotage with different production losses. Neither description overrides the selected C2C3 action handler.
- The base manual includes Plant Disease and the Conquests catalog does not. That difference does not authorize dropping or adding an action; inspect only the pinned C2C3 action/ruleset scope. For example, C2C3 defines Poison City separately.
- The manual describes Propaganda as city defection, with Democracy immunity and capital/Courthouse modifiers; Conquests Civilopedia confirms defection but does not repeat those modifiers. Use C2C3 Incite City enablers, effects, and handler behavior for mechanics; keep any unresolved unit-free presentation mapping explicit.

## Suggested UI and API boundary

Keep mission execution in a diplomacy/espionage service and represent persistent assets separately from units:

```text
DiplomacyState
  relations[player][opponent].embassy: EmbassyState
  relations[player][opponent].spy: SpyAssetState

C2C3DiplomacyAction
  action: selected Freeciv ActionId
  target: civilization, city, unit, or none, as defined by that action

available_actions(player, opponent, game_state) -> ActionAvailability[]
execute_action(player, opponent, action, target?) -> ActionResult
```

The action IDs, targets, enablers, costs, random rolls, and outcomes come from the effective selected C2C3 ruleset and pinned Freeciv handlers. The Civ III-facing screen can group or rename those actions, but its Conquests catalog and risk controls do not alter their mechanics. Availability must use the shared action-enabler evaluator; execution must use shared effect/action resolution rather than parallel formulas.

P10 does not implement a screen actor adapter or espionage action eligibility. The selected C2C3 ruleset retains its action records as data, but future espionage work must decide how its unit actor requirements map to the Civ III-facing interface. The evaluator distinguishes a real embassy from Marco Polo's broader access for other ruleset checks.

No espionage eligibility or execution adapter is implemented. The native Freeciv actions also require a map actor at distance 0–1 from the target city (`common/actions.c::action_prob_possible`, pinned revision `9ff184b6eb128ca636c7ad6269e27aa959600006`). A unit-free screen has no native actor location, so locality remains an unresolved compatibility question for the separately scoped work.

A future UI could expose opponent selection, embassy/spy state, mapped C2C3 action choices, and a target selector when an action requires one. Conquests screen descriptions are interface guidance only. Do not add Freeciv-only unit identities or movement controls.

Eligibility should be derived from game state and selected C2C3 action enablers rather than stored as UI-only availability. Map Civ III-compatible embassy/spy state to the actor requirements those enablers use. Do not add Writing, Espionage, Intelligence Agency, treasury, or mission gates solely because a Civ III source describes them; include them only where the selected C2C3 ruleset requires them.

## Unit-free compatibility questions

The selected C2C3 ruleset and handlers specify gameplay costs, probabilities,
enablers, and outcomes. These are implementation inputs, not open design
choices. The remaining questions concern adapting their unit-actor model while
keeping the Civ III unit catalog. These questions are deferred and are not
P10 blockers or decisions already made:

1. Which unit-free C2C3 actor profile supplies each actor requirement, including `Diplomat`/`Spy` flags and movement-point requirements. Actor location and native 0–1 target range have no direct screen equivalent and need an explicit adapter decision.
2. How to represent Freeciv actor consumption, capture, escape, and detection without adding Freeciv-only unit kinds or map movement, while preserving C2C3 outcomes and deterministic random-call order.
3. How each C2C3 action target and result maps to the Civ III-facing interface, especially actions whose native handler expects a unit actor, a unit target, or a stack target.

Resolve these adapter questions against the pinned C2C3 ruleset, handlers, and the Civ III unit catalog. Keep any compatibility mapping explicit and data-driven; do not substitute Conquests mission mechanics or another Freeciv ruleset.

## Evidence consulted

- Repository manual: [`civ3_manual.pdf`](civ3_manual.pdf), Chapter 8 (printed p. 38) and Chapter 13 (printed pp. 138-49). Accessible copy used for page/text cross-check: [Civilization III Manual](https://usermanual.wiki/Document/Civilization3Manual.1841378190.pdf).
- Conquests Civilopedia: `../../source/Conquests/Text/Civilopedia.txt`, entries `GCON_Espionage` / `DESC_GCON_Espionage` (lines 817-71), `GCON_Espionage_Missions` (lines 876-28), `GCON_Embassies` (lines 1607-20), `BLDG_Intelligence_Center` (around line 5762), and `TECH_Espionage` (around line 6535).
- Base-game Civilopedia: `../../source/Text/Civilopedia.txt`, `GCON_Espionage` (lines 631-63) and `GCON_Embassies` (lines 1302-15), used only to identify edition differences.
