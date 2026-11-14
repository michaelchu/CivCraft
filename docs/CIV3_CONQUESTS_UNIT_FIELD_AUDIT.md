# Civ III: Conquests PRTO field audit

## Scope and evidence

This audit compares every source-backed field currently exported in
`data/rulesets/civ3_conquests/units.tsv` with the installed
`../../source/Conquests/conquests.biq` and the adjacent OpenCiv3 BIQ reader at
`../openciv3/QueryCiv3/BiqSections/Prto.cs`. The installed BIQ SHA-256 is
`1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c`.
`/tmp/conquests.unpacked` is the current BIQ's decompressed data; the packed
source file and its digest were checked directly.

OpenCiv3's `Biq.cs` declares a 238-byte PRTO prefix plus a 21-byte suffix,
with stealth-target PRTO indices stored between them; its reader copies those
regions according to `NumberOfStealthTargets`. The exporter checks that each
decoded record length equals `238 + 4 * target_count + 21` payload bytes. The
Prima editor guide defines Zone of Control and core unit fields. A CivFanatics
editor reference describes additional settings such as Air Defense, Worker
Strength, Require Support, bombard effects, and enslave results. Since that
second reference is community material, raw values are preserved without
converting uncertain semantics into simulation behavior.

The PRTO struct confirms the fixed field layout used by the exporter:

| BIQ PRTO field(s) | TSV column(s) | Audit result |
|---|---|---|
| Prototype name and source record index | `name`, `source_prototypes`, `source_biq` | Correctly identifies the selected prototype and retains all same-name prototype indices. |
| Bombard strength/range, capacity, shield cost, defense, attack, operational range, population cost, rate of fire, movement | Corresponding numeric columns | Correctly maps PRTO's 16 consecutive integers beginning at offset +72, including `IconIndex`. |
| Required technology and three resources | `required_technology`, `required_resource_1..3` | Correctly resolves TECH and GOOD indices to names. |
| Upgrade prototype index | `upgrade_to` | Correctly resolves the index back to a PRTO name. |
| Flags 1/2/3, type, hit-point bonus, strategy, civilization mask, terrain movement mask | Corresponding raw columns | Correct source offsets; flags 2 is retained as raw hex without a semantic decoder. Source AI profiles retain each same-name row's strategy and full flags 1. |
| Zone of Control, Bombard Effects, Require Support, Enslave Results, stealth-target PRTO indices, Create Craters, Worker Strength, Air Defense, Icon Index, Civilopedia Entry | Corresponding columns appended after `ignore_movement_cost_hex` | Raw values are preserved in the generated TSV and parsed rule record. Enslave/stealth references remain numeric PRTO indices so duplicated and scenario-only prototypes are not conflated with campaign identities. |
| Source-derived unique/standard/wonder categorization, civilization replacement, art mapping, expansion label | `category`, `unique_civilization`, `replaces`, `art_folder`, `art_root`, `expansion` | These are curated/catalog mappings rather than PRTO fields. The 31 unique mappings and two wonder-created identities agree with the audited roster notes. Expansion for standard identities is inferred from prototype index thresholds, not read from PRTO. |

An independent direct decode compared every source-derived TSV field across
all 92 campaign identities. This includes 2,300 comparisons for the original
25 exported PRTO fields and 920 additional comparisons for the ten newly
preserved fields, including dynamic stealth-target IDs. Regenerating the TSVs with the checked-in exporter
and current unpacked BIQ exactly matches the checked-in unit,
civilization-start, and experience-level tables. No incorrect decoding was
found among the columns currently exported.

## Preserved PRTO fields with behavior still unresolved

The exporter now retains the listed source fields, but simulation behavior is
not yet fully implemented. Offsets below are relative to the start of the PRTO
record; the record length excludes the leading four-byte length field. Counts
are across all 141 installed PRTO prototypes, including duplicate identities
and special/scenario records, so they describe source presence rather than
only the 92-row campaign catalog.

| Preserved source field | PRTO offset / representation | Source observation | Remaining runtime work |
|---|---|---|---|
| `ZoneOfControl` | +4, integer | Value 1 on 16 prototypes (14 campaign identities); zero on the rest. | The Prima editor guide defines this as a unit zone that automatically attacks passing enemies. Raw value is preserved; reaction movement/combat remains unimplemented. |
| `IconIndex` | +92, integer (sixth value in the +72 block) | Nonzero on 140 prototypes. | Raw value is preserved for UI/catalog consumers; runtime continues to use mapped unit art. |
| `BombardEffects` | +188, integer | Value 1 on 16 prototypes (15 campaign identities). | Implemented through the existing combat visual event: flagged bombardment shows a target explosion during its final effect frames, whether or not it kills the selected unit. The editor reference defines the field as enabling explosion animations; this runtime overlay is a project-native representation, not original Civ III sprite playback. |
| `RequireSupport` | +206, integer | Value 1 on 110 prototypes and zero on 31; all 92 campaign identities have value 1. | Raw value is preserved. The editor reference says this controls whether the unit needs money or city support; its interaction with Conquests government thresholds still needs behavior-level verification. |
| `EnslaveResults` | +226, integer | Non-default values on Man-O-War (PRTO[63]), Privateer (PRTO[65]), and Javelin Thrower (PRTO[1], Worker); the repeated Javelin Thrower prototype produces the fourth non-default record. | On attacker victory, runtime rolls the Conquests Civilopedia's one-in-three chance, resolves the configured PRTO result index to a campaign identity, and creates that unit at the battle tile under the victor's ownership. The new unit keeps the defeated unit's nationality; this follows the manual's captured-unit nationality rule as an inference because the Enslavement entry does not state nationality directly. This combat conversion remains separate from ordinary movement capture. |
| Ordinary unit capture (`UnitFlag::Capturable` / `Capturer`) | No corresponding PRTO property; `Flags3[6]` bit 0 is `StealthAttack`, and `EnslaveResults` is a separate field at +226. | Conquests manual says military units with an attack factor capture undefended units; it specifically includes artillery and notes the required technology. Captured Settlers become two Workers. This is distinct from Enslavement, whose 1-in-3 success chance and replacement unit are described in Conquests Civilopedia. | Runtime now derives capturers from positive attack and capturable units from zero attack/defense (excluding missile-class units), checks the captured unit's required technology, transfers the capturer into the tile, and converts Settlers into two Workers retaining their original nationality. It only captures a stack when every unit there is eligible; mixed stacks with a real defender proceed through ordinary combat. These are manual-backed derived rules, not explicit BIQ flags. |
| `NumberOfStealthTargets` and dynamic target PRTO indices | +234, count followed by that many 32-bit indices | Seven prototypes have target lists: Submarine (18), Nuclear Submarine (18), Stealth Fighter (92), Stealth Bomber (91), and F-15 (91); repeated source prototypes account for the remaining two rows. | The semicolon-separated source-index list is checked against each defender identity's imported `source_prototypes` indices when combat chooses a defender. Duplicate BIQ identities are eligible if any imported index matches. Runtime still chooses the best eligible defender automatically; the manual's player-facing stealth-target selection box is not implemented. PRTO record length varies with target-list length. |
| `CreateCraters` | In the 21-byte fixed tail after the dynamic stealth-target list | Set on 19 prototypes (18 campaign identities), including artillery and several aircraft/naval bombarders. | Implemented as persistent crater damage when a flagged bombard destroys the last unit on a non-city tile with no remaining terrain improvements. It reduces food, shields, and trade by one and Workers can clear it. The Conquests Civilopedia also permits crater creation after the last terrain improvement is destroyed, but terrain-improvement bombard destruction is not implemented here. Its wording says a crater “may” form; the exact chance is not specified by the sources read, and this implementation currently treats qualifying states as certain. |
| `WorkerStrength` | Float in that same fixed tail | Exactly 1.0 on Worker, Crusader, and two TOW Infantry prototypes; zero elsewhere. This is three campaign identities because TOW Infantry has a repeated source prototype. | Foreign Worker progress now uses the imported Worker value relative to native baseline 2: 1.0 yields half the normal rate, matching Prima's guide (p. 108). The manual says captured Workers retain nationality and captured Settlers become two Workers (pp. 56–57); captured Settlers' converted Workers preserve nationality and use the foreign rate. Worker alone is the standard campaign Worker identity. Crusader has a separate Build Fortress action but is not a Worker type and cannot be captured or traded in the standard campaign; TOW Infantry has no worker action flags. Their 1.0 values therefore do not affect foreign Worker progress. `activity_rate` is shared by actual progress and projected completion. |
| `AirDefense` | Integer at the end of that fixed tail | Nonzero on Carrier (1), Destroyer (1), Battleship (2), AEGIS Cruiser (3), Cruiser (1), Flak (2), and Mobile SAM (4). | Implemented for aircraft/missile bombardment: up to four defending units roll independently against bomber defense; a successful roll destroys the bomber and cancels its bombard. The Conquests FAQ confirms Flak/Mobile SAM can attack aircraft bombarding their tile, and community combat reports supply the one-tenth-strength and `A/(A+D)` formula. This interpretation is empirical community guidance, not an official formula. Fighter Air Superiority and city SAM Battery interception are separate mechanics and remain outside this change. |
| `UnknownBuffer`, `UnknownBuffer2`, `UnknownBuffer3`, `UnknownBuffer4` | Reserved byte regions in the reader struct | Not interpreted by the installed OpenCiv3 parser. | Unknown data is intentionally not decoded. A raw-record archival export would be needed to preserve it losslessly. |

The only named parser fields not exported are the reader's reserved/unknown
buffers. Flags 2 is preserved raw in `flags_2_hex` but its bits lack an audited
semantic mapping. Flags 1 and 3 are likewise preserved raw.
`ignore_movement_cost_hex` is decoded from the 14-byte PRTOTERR mask after
`BombardEffects` and contains the Chasqui Scout hills/mountains flags.

The per-unit Air Defense behavior is informed by the [Conquests FAQ](https://civfanatics.com/civ3/faq/)
and the [community-tested AA combat formula](https://forums.civfanatics.com/threads/flak.116569/).
The FAQ confirms AA units can attack planes bombing their tile; the formula
reference reports effective strength at one tenth of the editor value, at most
four AA units per tile, and per-unit kill probability `A/(A+D)`. The BIQ field
is scaled against the simulation's `POWER_FACTOR` so its probability matches
that ratio. This is not a claim that the official manual publishes the exact
probability formula.

## Source verification

The extraction utility is no longer distributed with CivCraft. During the
original audit, all three generated catalogs matched their checked-in files and
the SHA-256 matched the source
provenance in the TSV. The 3,220 direct field comparisons were an inline Python audit
against the same unpacked bytes and the PRTO struct offsets above. Focused Air
Defense behavior checks ran with `cargo test --offline air_defense`; both
interception and pass-through cases passed. No unrelated tests were run.

## Follow-up

The editable table now retains all named PRTO fields identified by the
OpenCiv3 reader, though several raw values still need semantic/runtime work.
`ZoneOfControl` now gates movement control; reaction attacks and the other
listed mechanics still need source-backed implementation. Unit Air Defense is
implemented for tile bombardment with the community-tested formula above.
Resolve unknown-field meanings, flags-2 semantics, and curated expansion
metadata separately. The manual and Civilopedia remain useful for behavior
and naming, but the installed Conquests BIQ is authoritative for this edition's
PRTO values.
