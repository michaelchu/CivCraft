# Civ III Conquests espionage implementation

## Target and design

Target standard Civilization III: Conquests 1.22, using the campaign data in
`source/Conquests/conquests.biq`. The user selected the Conquests espionage
screen model: embassy and Spy are persistent player-to-player assets, and
operations are launched from a UI. No Spy or Diplomat map unit is added.
Freeciv action behavior is not the target.

The client opens Espionage after Writing, lists opponents with unmet status
where appropriate, shows embassy and Spy status, and submits missions through
`Command::Espionage`.
Mission results and eligibility belong to `Simulation`. The current screen
offers embassy establishment, city investigation, technology theft, Spy
planting, map/plans theft, production sabotage, propaganda, and counterspy.
Risk selection, target city selection, treasury costs, reports, and acquired
military-plan locations are represented in the UI.

## Source-backed behavior

- The base-game official manual says embassies follow Writing and contact,
  allow city investigation and technology theft, close during war, and reopen
  when peace is restored (Chapter 13, printed pp. 138–139). It describes
  espionage operations, detection, and international incidents separately
  from mission success (pp. 145–149). The Conquests Complete manual describes
  the five-region Espionage screen, opponent and city selectors, risk choices,
  and mission flow (Chapter 13, pp. 166–167).
- The installed Conquests Civilopedia (`source/Conquests/Text/Civilopedia.txt`)
  describes the post-Writing screen and embassy, the Espionage/Intelligence
  Agency gates, Spy status, mission catalog, operational costs, and Spy
  experience (entries `GCON_Espionage`, `GCON_Espionage_Missions`,
  `GCON_Embassies`, `BLDG_Intelligence_Center`, and `TECH_Espionage`).
- Supplementary reverse-engineered costs, escape chances, and propaganda
  formulas are catalogued in `CIV3_MANUAL_RULES.md`. The game manual does not
  publish these formulas. Evidence comes from CivFanatics and CivWiki; use the
  installed Conquests campaign and runtime as the final tie-breaker where they
  disagree.

## Implemented state

- Embassy, planted Spy experience, failed-plant record, and military-plan
  expiry are stored on the directed civilization-pair relation and serialized
  with the campaign. Campaign save header advanced for the state addition.
- Embassy establishment and city investigation report capital/city assets.
  Theft/sabotage missions distinguish success from discovery, charge gold,
  and can trigger a diplomatic incident. Embassies can perform the missions
  for which the reference sources permit embassy operations; covert missions
  can also use a planted Spy.
- Planting is blocked at war and requires an embassy, Espionage, an Intelligence
  Agency, and no existing Spy. Communism/Fascism grant veteran Spy status. The
  failed-plant marker now rolls its 1-in-3 decay chance on each turn boundary.
- Propaganda checks Democracy immunity, government/culture modifiers, garrison,
  Courthouse, Palace, veteran Spy, and human-national citizens. Failed
  propaganda stores confused citizens as unhappy and decays that count over
  subsequent city turns.
- The Espionage panel submits all operations through simulation commands; UI
  code does not mutate campaign state. City and military-plan selectors page
  through the full set of targets and reported units.
- AI civilizations now establish embassies, plant Spies, and choose from
  technology/map/plans theft, production sabotage, propaganda, and
  counterespionage through the same operation executor. Discovered operations
  create a diplomatic incident; AI targets lose trust and may schedule war
  through the existing diplomacy planner.

## Unresolved and limited fidelity

- The Carefully multiplier is implemented as a 50% surcharge (1.5x), matching
  CivWiki's explicit value and interpreting the CivFanatics study's “multiply
  by 50%” as a surcharge. The specific Conquests executable outcome has not
  been measured.
- Technology theft uses the Standard world-size tech rate of 240 from the
  standard Conquests world-size settings; custom world-size selection is not
  represented in CivCraft's current campaign model.
- Propaganda uses the documented cultural-opinion ratios (3:1, 2:1, 1:1,
  3:4, 1:2, 1:3) and their 30/25/20/10/5/3 modifiers. These thresholds come
  from Prima's PTW guide and have not been independently compared with the
  Conquests 1.22 executable.
- A detected operation lowers the AI target's trust and can trigger its
  existing war planner. The manual says the target is likely to treat the
  incident as an act of war, so the AI does not declare war automatically.
- Investigation and operation output is shown in campaign events; exact
  Conquests risk descriptions and incident response wording are not yet
  reproduced.

## Evidence

- `docs/civ3_manual.pdf`, Chapter 13, printed pp. 138–149; [official manual
  copy](https://cdn.akamai.steamstatic.com/steam/apps/3910/manuals/manual.pdf?tI=),
  Chapter 13, pp. 166–167 for the screen.
- Installed Conquests Civilopedia at
  `../../source/Conquests/Text/Civilopedia.txt`; lines 817–928, 1607–1620,
  5762–5765, and 6535–6539 (Conquests campaign data supplied with this
  checkout).
- CivFanatics, “A Study of Espionage Missions,”
  <https://civfanatics.com/civ3/strategy/game-mechanics/a-study-of-espionage-missions/>.
  Reverse-engineered cost, detection, planting, and propaganda claims; page
  does not pin its edition or patch.
- CivWiki, “Spionageaktionen (Civ3),”
  <https://civ-wiki.de/wiki/Spionageaktionen_%28Civ3%29>. Secondary summary
  with citations to Prima and community analysis; page does not pin its patch.
- Prima Games, *Civilization III: Play the World Advanced Strategies*, printed
  pp. 56–57, “Effects of Culture,”
  <https://oldapplestuff.com/download/Macintosh/Macintosh_Garden/manuals/Civilization_III_PTW_strategy_guide.pdf>.
  This is the source for cultural-opinion ratio thresholds; its applicability
  to Conquests is supplementary evidence, not an executable confirmation.
- CivWiki, “Forschungskosten (Civ3),”
  <https://civ-wiki.de/wiki/Forschungskosten_%28Civ3%29>. Secondary source
  listing a 240 map modifier for Standard-size games; used only for the
  Standard campaign's espionage tech-rate baseline.
- Claims checked 2026-11-14. Preserve conflicting evidence and unresolved
  formulas rather than treating either community source as official data.
