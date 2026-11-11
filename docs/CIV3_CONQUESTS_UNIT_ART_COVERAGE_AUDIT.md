# Civ III: Conquests Campaign Unit Art Coverage Audit

## Scope and evidence

This audit compares the 92 active rows in [`data/rulesets/civ3_conquests/units.tsv`](../data/rulesets/civ3_conquests/units.tsv) with the checked-in original art trees: `../../source/Art/Units`, `../../source/civ3PTW/Art/Units`, and `../../source/Conquests/Art/Units`. Folder and file matching was case-insensitive to reflect the source game’s Windows paths. This is filesystem evidence, not a claim that CivCraft currently loads every source animation correctly.

Roster counts in the TSV: **92 identities** (59 standard, 31 unique, 2 wonder-created). All 92 rows resolve to at least one source unit folder and an `.ini` animation manifest. The Leader and Army rows each use four era-specific folders, so their mappings cover 8 folders; the other 90 rows each name one folder. The catalog lists the two Paratrooper identities separately but both point to a folder named `Paratrooper`; preserve the distinction in rules and determine the intended era/root per identity.

## Per-identity folder and manifest mapping

“Found in” lists actual matching folders among the three roots. `TSV art root` is the current data row’s declared lookup root; the folder listing is independent filesystem verification. The two era-family rows show all folder variants in the source order.

| ID | Type | TSV art folder(s) | TSV art root(s) | Found in (folder / `.ini`) |
|---|---|---|---|
| `settler` | standard | Settler | base | `base/Settler/` (`settler.ini`) |
| `worker` | standard | Worker | base | `base/Worker/` (`Worker.INI`) |
| `scout` | standard | Scout | base | `base/Scout/` (`Scout.INI`) |
| `explorer` | standard | Explorer | base | `base/Explorer/` (`Explorer.ini`) |
| `marine` | standard | Marine | base | `base/Marine/` (`Marine.INI`) |
| `modern_paratrooper` | standard | Paratrooper | base | `base/Paratrooper/` (`Paratrooper.INI`) ; `conquests/Paratrooper/` (`Paratrooper.INI`) |
| `warrior` | standard | Warrior | base | `base/warrior/` (`Warrior.INI`) |
| `archer` | standard | Archer | base | `base/Archer/` (`Archer.INI`) |
| `spearman` | standard | Spearman | base | `base/Spearman/` (`Spearman.INI`) |
| `swordsman` | standard | Swordsman | base | `base/Swordsman/` (`Swordsman.INI`) ; `ptw/Swordsman/` (`Swordsman.INI`) |
| `chariot` | standard | Chariot | base | `base/chariot/` (`Chariot.INI`) |
| `horseman` | standard | Horseman | base | `base/Horseman/` (`Horseman.INI`) |
| `pikeman` | standard | Pikeman | base | `base/Pikeman/` (`Pikeman.ini`) |
| `longbowman` | standard | Longbowman | base | `base/Longbowman/` (`Longbowman.INI`) |
| `musketman` | standard | Musketman | base | `base/Musketman/` (`Musketman.ini`) |
| `knight` | standard | Knight | base | `base/Knight/` (`Knight.INI`) |
| `rifleman` | standard | Rifleman | base | `base/Rifleman/` (`Rifleman.INI`) |
| `cavalry` | standard | Cavalry | base | `base/Cavalry/` (`Cavalry.INI`) |
| `infantry` | standard | Infantry | base | `base/Infantry/` (`Infantry.INI`) ; `ptw/Infantry/` (`Infantry.INI`) |
| `tank` | standard | Tank | base | `base/Tank/` (`Tank.ini`) |
| `mech_infantry` | standard | Mech Infantry | base | `base/Mech Infantry/` (`Mech Infantry.ini`) |
| `modern_armor` | standard | Modern Armor | base | `base/Modern Armor/` (`Modern Armor.ini`) |
| `catapult` | standard | Catapult | base | `base/Catapult/` (`Catapult.INI`) ; `ptw/Catapult/` (`Catapult.INI`) |
| `cannon` | standard | Cannon | base | `base/Cannon/` (`Cannon.INI`) ; `ptw/Cannon/` (`Cannon.INI`) |
| `artillery` | standard | Artillery | base | `base/Artillery/` (`Artillery.INI`) |
| `radar_artillery` | standard | Radar Artillery | base | `base/Radar Artillery/` (`Radar Artillery.INI`) |
| `cruise_missile` | standard | Cruise Missile | base | `base/Cruise Missile/` (`Cruise Missile.INI`) |
| `tactical_nuke` | standard | Tactical Nuke | base | `base/Tactical Nuke/` (`Tactical Nuke.INI`) ; `ptw/Tactical Nuke/` (`tactical nuke.ini`) |
| `icbm` | standard | ICBM | base | `base/ICBM/` (`ICBM.INI`) ; `ptw/ICBM/` (`icbm.ini`) |
| `galley` | standard | Galley | base | `base/Galley/` (`Galley.ini`) |
| `caravel` | standard | Caravel | base | `base/Caravel/` (`Caravel.ini`) ; `conquests/Caravel/` (`Caravel.ini`) |
| `frigate` | standard | Frigate | base | `base/Frigate/` (`Frigate.ini`) |
| `galleon` | standard | Galleon | base | `base/Galleon/` (`Galleon.ini`) |
| `ironclad` | standard | Ironclad | base | `base/Ironclad/` (`Ironclad.ini`) |
| `transport` | standard | Transport | base | `base/Transport/` (`Transport.ini`) |
| `carrier` | standard | Carrier | base | `base/Carrier/` (`Carrier.ini`) |
| `submarine` | standard | Submarine | base | `base/Submarine/` (`Submarine.INI`) |
| `destroyer` | standard | Destroyer | base | `base/Destroyer/` (`Destroyer.ini`) ; `conquests/Destroyer/` (`Destroyer.ini`) |
| `battleship` | standard | Battleship | base | `base/Battleship/` (`Battleship.INI`) |
| `aegis_cruiser` | standard | Aegis Cruiser | base | `base/Aegis Cruiser/` (`AEGIS Cruiser.ini`) |
| `nuclear_submarine` | standard | Nuclear Submarine | base | `base/Nuclear Submarine/` (`Nuclear Submarine.INI`) |
| `fighter` | standard | Fighter | base | `base/Fighter/` (`Fighter.INI`) |
| `bomber` | standard | Bomber | base | `base/Bomber/` (`Bomber.INI`) |
| `helicopter` | standard | Helicopter | base | `base/Helicopter/` (`Helicopter.INI`) |
| `jet_fighter` | standard | Jet Fighter | base | `base/Jet Fighter/` (`Jet Fighter.INI`) |
| `stealth_fighter` | standard | Stealth Fighter | base | `base/Stealth Fighter/` (`Stealth Fighter.INI`) |
| `stealth_bomber` | standard | Stealth Bomber | base | `base/Stealth Bomber/` (`Stealth Bomber.INI`) |
| `leader` | standard | Leader Ancient Times\|Leader Middle Ages\|Leader Industrial Ages\|Leader Modern Times | base\|base\|base\|base | `base/Leader Ancient Times/` (`Leader Ancient Times.ini`) <br> `base/Leader Middle Ages/` (`Leader Middle Ages.ini`) <br> `base/Leader Industrial Ages/` (`Leader Industrial Ages.ini`) <br> `base/Leader Modern Times/` (`Leader Modern Times.ini`) |
| `army` | standard | Army Ancient Times\|Army Middle Ages\|Army Industrial Ages\|Army Modern Times | base\|base\|base\|base | `base/Army Ancient Times/` (`Army Ancient Times.ini`) <br> `base/Army Middle Ages/` (`Army Middle Ages.ini`) <br> `base/Army Industrial Ages/` (`Army Industrial Ages.ini`) <br> `base/Army Modern Times/` (`Army Modern Times.ini`) |
| `jaguar_warrior` | unique | Jaguar Warrior | base | `base/Jaguar Warrior/` (`Jaguar Warrior.INI`) |
| `bowman` | unique | Bowman | base | `base/Bowman/` (`Bowman.INI`) |
| `hoplite` | unique | Hoplite | base | `base/Hoplite/` (`Hoplite.INI`) |
| `impi` | unique | Impi | base | `base/Impi/` (`Impi.INI`) |
| `legionary` | unique | Legionary | base | `base/Legionary/` (`Legionary.INI`) ; `conquests/Legionary/` (`Legionary.INI`) |
| `immortals` | unique | Immortals | base | `base/Immortals/` (`Immortals.INI`) |
| `war_chariot` | unique | War Chariot | base | `base/War Chariot/` (`War Chariot.INI`) |
| `rider` | unique | Rider | base | `base/Rider/` (`Rider.INI`) |
| `mounted_warrior` | unique | Mounted Warrior | base | `base/Mounted Warrior/` (`Mounted Warrior.INI`) |
| `musketeer` | unique | Musketeer | base | `base/Musketeer/` (`Musketeer.ini`) |
| `samurai` | unique | Samurai | base | `base/Samurai/` (`Samurai.INI`) |
| `war_elephant` | unique | War Elephant | base | `base/War Elephant/` (`War Elephant.INI`) |
| `cossack` | unique | Cossack | base | `base/Cossack/` (`Cossack.INI`) |
| `panzer` | unique | Panzer | base | `base/Panzer/` (`Panzer.ini`) |
| `man_o_war` | unique | Man-O-War | base | `base/Man-O-War/` (`Man-O-War.INI`) |
| `f_15` | unique | F-15 | base | `base/F-15/` (`F-15.INI`) ; `ptw/F-15/` (`F-15.INI`) |
| `privateer` | standard | Privateer | base | `base/Privateer/` (`Privateer.ini`) |
| `keshik` | unique | Keshik | ptw | `ptw/Keshik/` (`keshik.ini`) |
| `conquistador` | unique | Conquistador | ptw | `ptw/Conquistador/` (`conquistador.ini`) |
| `berserk` | unique | Berserk | ptw | `ptw/Berserk/` (`berserk.ini`) |
| `sipahi` | unique | Sipahi | ptw | `ptw/Sipahi/` (`Sipahi.ini`) |
| `gallic_swordsman` | unique | Gallic Swordsman | ptw | `ptw/Gallic Swordsman/` (`gallic swordsman.ini`) |
| `ansar_warrior` | unique | Ansar Warrior | ptw | `ptw/Ansar Warrior/` (`ansar warrior.ini`) |
| `numidian_mercenary` | unique | Libyan Mercenary | ptw | `ptw/Libyan Mercenary/` (`Libyan Mercenary.ini`) |
| `hwach_a` | unique | Hwacha | ptw | `ptw/Hwacha/` (`Hwacha.ini`) |
| `medieval_infantry` | standard | Medieval Infantry | ptw | `ptw/Medieval Infantry/` (`Medieval Infantry.ini`) |
| `guerilla` | standard | Guerilla | ptw | `ptw/Guerilla/` (`Guerilla.INI`) |
| `enkidu_warrior` | unique | Enkidu Warrior | conquests | `conquests/Enkidu Warrior/` (`Enkidu Warrior.INI`) |
| `three_man_chariot` | unique | Three Man Chariot | conquests | `conquests/Three Man Chariot/` (`Three Man Chariot.INI`) |
| `carrack` | unique | Carrack | conquests | `conquests/Carrack/` (`Carrack.ini`) |
| `swiss_mercenary` | unique | Swiss Mercenary | conquests | `conquests/Swiss Mercenary/` (`Swiss Mercenary.ini`) |
| `trebuchet` | standard | Trebuchet | conquests | `conquests/Trebuchet/` (`Trebuchet.ini`) |
| `chasqui_scout` | unique | Chasquis Scout | conquests | `conquests/Chasquis Scout/` (`Chasquis Scout.INI`) |
| `javelin_thrower` | unique | Javelin Thrower | conquests | `conquests/Javelin Thrower/` (`Javelin Thrower.ini`) |
| `dromon` | unique | Dromon | conquests | `conquests/Dromon/` (`Dromon.ini`) |
| `cruiser` | standard | Cruiser | conquests | `conquests/Cruiser/` (`Cruiser.ini`) |
| `crusader` | wonder_created | Crusader | conquests | `conquests/Crusader/` (`Crusader.ini`) |
| `ancient_cavalry` | wonder_created | Ancient Cavalry | conquests | `conquests/Ancient Cavalry/` (`Ancient Cavalry.ini`) |
| `curragh` | standard | Curragh | conquests | `conquests/Curragh/` (`Curragh.ini`) |
| `paratrooper` | standard | Paratrooper | conquests | `base/Paratrooper/` (`Paratrooper.INI`) ; `conquests/Paratrooper/` (`Paratrooper.INI`) |
| `tow_infantry` | standard | TOW Infantry | conquests | `conquests/TOW Infantry/` (`TOW Infantry.INI`) |
| `flak` | standard | Flak | conquests | `conquests/Flak/` (`Flak.INI`) |
| `mobile_sam` | standard | Mobile SAM | conquests | `conquests/Mobile SAM/` (`Mobile SAM.INI`) |

## Cross-expansion and naming findings

- Several common-unit folders are duplicated in later expansion trees: `Paratrooper`, `Swordsman`, `Infantry`, `Catapult`, `Cannon`, `Caravel`, `Destroyer`, `Legionary`, and `F-15`. For these, do not assume that a folder name identifies one physical asset set; the declared root and selected `.ini` matter.
- The Conquests `Paratrooper/Paratrooper.INI` adds `BUILD=ParatrooperDrop.flc`; the base `Paratrooper/Paratrooper.INI` leaves `BUILD` empty. The regenerated catalog now distinguishes the identities by root: `modern_paratrooper` selects base art and `paratrooper` selects Conquests art. Both folders remain named `Paratrooper`, so keep the root component in the identity mapping; folder name alone is ambiguous.
- Data-to-source naming differences are intentional aliases in the current table: `numidian_mercenary` → `Libyan Mercenary`; `chasqui_scout` → `Chasquis Scout`; `hwach_a` → `Hwacha`; `aegis_cruiser` → `Aegis Cruiser`; `three_man_chariot` → `Three Man Chariot`.
- `leader` and `army` are single catalog identities with four age-specific folders each: Ancient Times, Middle Ages, Industrial Ages, and Modern Times. The runtime must select the art variant by the unit’s era/age state; one fixed folder per identity is insufficient.

## Animation manifest and shared-file audit

Each mapped folder has an `.ini` file. Parsing the nonblank `[Animations]` entries across all row-to-folder/root matches found **707 animation-file references** (622 distinct referenced filenames). Every referenced filename exists somewhere in the three campaign art trees. **55 references are not present inside the manifest’s own expansion art root**, but matching assets exist in the base `Art/Units` tree; this confirms that later expansion manifests depend on shared base assets and the loader needs an ordered cross-root fallback. One `Rider` manifest uses `DEAD=.`; treat this as a sentinel/empty value rather than a filename.

The manifests declare animation states and per-state timing in `[Animations]` and `[Timing]`. The number of keys varies by manifest (20, 23, 26, or 29), so a hard-coded partial state list will lose source behavior. This audit verified referenced-name existence but did not decode every FLC, compare frame counts, or verify playback timing in the engine. Those remain implementation/verification work.

## Unmapped folders: keep outside the campaign roster pending rules evidence

The following folders exist in these top-level campaign art roots but are not directly named by the 92-row catalog. They are candidates for scenario, special-mode, or alternate-age handling; folder presence alone does not establish that a folder is an additional campaign unit. Base `Settler/Worker Industrial Ages/Modern Times` are era variants, not evidence for extra unit identities. `Palettes` is an art resource directory, not a unit.

### base (`../../source/Art/Units`)

`Palettes`, `Settler Industrial Ages`, `Settler Modern Times`, `Worker Industrial Ages`, `Worker Modern Times`.

### ptw (`../../source/civ3PTW/Art/Units`)

`King`, `King American`, `King Arab`, `King Aztec`, `King Babylonian`, `King Carthaginian`, `King Celtic`, `King Chinese`, `King Egyptian`, `King English`, `King French`, `King German`, `King Greek`, `King Indian`, `King Iroquois`, `King Japanese`, `King Korean`, `King Mongol`, `King Ottoman`, `King Persian`, `King Roman`, `King Russian`, `King Spanish`, `King Viking`, `King Zulu`, `Princess`, `The King`, `Worker Modern Times`.

### conquests (`../../source/Conquests/Art/Units`)

`Enslaved Worker`, `Holy Relic`, `Hussar`, `Inquisitor`, `King Austria`, `King Byzantines`, `King Hatti`, `King Incan`, `King Mayan`, `King Netherlands`, `King Portugal`, `King Sumeria`, `Ninja`, `SciLeader`, `Shogun`, `WWII Paratrooper`.

## Source files to inspect next

- Per-unit animation definitions: each path in the table, especially duplicate expansion manifests and the four Leader/Army era variants.
- Main campaign unit definitions: `../../source/Conquests/Conquests/conquests.biq` and any expansion BIQ records referenced by the TSV’s `source_biq` column. Use the BIQ `Art`/unit data to resolve shared-folder aliases and distinguish campaign units from scenario-only support.
- Runtime loader: verify INI section/key parsing, source timing interpretation, shared-file fallback order, and palette handling against actual assets.
