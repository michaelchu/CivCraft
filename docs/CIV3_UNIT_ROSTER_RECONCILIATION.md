# Civ III unit roster reconciliation

## Purpose and evidence boundary

This inventory compares the simulation's current unit catalog with the
standard Civ III: Conquests campaign roster and the unit art shipped in this
checkout. The source art is the requested visual inventory; it does not by
itself establish that a unit is available in the standard ruleset, its
statistics, or its expansion status. In particular, Conquests contains
scenario-only art.

The standard roster was recovered from the installed `../../source/Conquests/conquests.biq` PRTO section. It contains 141 prototype rows because some types are copied for civilization-specific availability. Deduplicating names gives 90 identities with playable-civilization availability. Add the two standard wonder-created units, Ancient Cavalry and Crusader, which have no ordinary availability mask but are produced by wonders. That yields 92 standard campaign identities. This includes regular units, unique units, wonder-created units, and Conquests additions. It excludes scenario-only prototypes and named leader/Princess entities. Base art is under `../../source/Art/Units/`; Play the World art is under `../../source/civ3PTW/Art/Units/`; Conquests additions are under `../../source/Conquests/Art/Units/`. Scenario-pack art is nested under the expansion folders and should remain separately classified.

The installed `../../source/Text/Civilopedia.txt` and
`../../source/Conquests/Text/Civilopedia.txt` provide the in-game unit entries.
The Civ III manual at `civ3_manual.pdf` (Appendix A, printed pp. 201–205)
provides the vanilla unit chart and gameplay data. It predates the Conquests
expansion, so use it for base-unit mechanics and use the installed Conquests
BIQ/Civilopedia for the final target roster and Conquests changes. The active
code now maps the campaign roster to the source unit `.ini` files; see the
status sections below for the remaining rules and animation work.

As an online cross-check, the [CivFanatics vanilla unit index](https://civfanatics.com/civ3/civilopedia/units/), [Play the World unit index](https://civfanatics.com/civ3/civilopedia/play-the-world-units/), and [Conquests unit index](https://civfanatics.com/civ3/civilopedia/conquests-units/) separate common units, unique units, wonder units, ruler units, and scenario packs. These are secondary references; the installed BIQ and Civilopedia remain the roster evidence for this checkout.

## Current simulation catalog and migration status

`UnitKind::ALL_NATIVE` in `src/sim/model/units.rs` now lists all 92 standard
Conquests identities in source order. The source-backed `units.tsv` supplies
their stats, technology and resource requirements, civilization availability,
unique replacements, flags, and art folder/root mappings. The editable rules
input and export provenance are documented in
`data/rulesets/civ3_conquests/README.md`. `civilization_starts.tsv` contains
all 31 playable-civilization start profiles.

This is the active campaign catalog, not yet a complete purge of inherited
fallback stat/metadata arms. The runtime `UnitKind` now contains the 92
Conquests campaign identities; Diplomat and Spy are no longer unit identities.
Caravan and Freight identities, merchant AI, unit missions, save fields,
transport fixtures, city-pair trade routes, and Caravan wonder-shield state
have also been removed. Civ III's road, harbor, airport, diplomacy trade
network, and city-level espionage missions still need their Civ III-specific
implementations. Serialized enum indices intentionally no longer preserve the
old save layout.

`unit_available` and campaign production use Conquests availability rules.
This does not establish that every simulation behavior is Civ III accurate;
Freeciv-derived AI and other mechanics remain to be audited and replaced.

## Base art inventory

The base `../../source/Art/Units` directory has 76 directories other than `Palettes`.
Some directories are alternate-era looks for the same role, or visual forms
for special units, and therefore should not be counted as 76 distinct
ruleset units.

| Art directories | Count | Notes |
| --- | ---: | --- |
| Standard unit-named art | 62 | Includes regular units and unique units; see complete names below. |
| Army-era art | 4 | Army Ancient Times, Army Middle Ages, Army Industrial Ages, Army Modern Times. |
| Leader-era art | 4 | Leader Ancient Times, Leader Middle Ages, Leader Industrial Ages, Leader Modern Times. |
| Alternate Settler art | 2 | Settler Industrial Ages, Settler Modern Times. |
| Alternate Worker art | 2 | Worker Industrial Ages, Worker Modern Times. |
| Lowercase legacy-named art | 2 | `chariot`, `warrior`; likely art aliases/variants, not additional unit types. |

The complete directory list, excluding `Palettes`, is:

`Aegis Cruiser`, `Archer`, `Army Ancient Times`, `Army Industrial Ages`,
`Army Middle Ages`, `Army Modern Times`, `Artillery`, `Battleship`, `Bomber`,
`Bowman`, `Cannon`, `Caravel`, `Carrier`, `Catapult`, `Cavalry`, `Cossack`,
`Cruise Missile`, `Destroyer`, `Explorer`, `F-15`, `Fighter`, `Frigate`,
`Galleon`, `Galley`, `Helicopter`, `Hoplite`, `Horseman`, `ICBM`, `Immortals`,
`Impi`, `Infantry`, `Ironclad`, `Jaguar Warrior`, `Jet Fighter`, `Knight`,
`Leader Ancient Times`, `Leader Industrial Ages`, `Leader Middle Ages`,
`Leader Modern Times`, `Legionary`, `Longbowman`, `Man-O-War`, `Marine`,
`Mech Infantry`, `Modern Armor`, `Mounted Warrior`, `Musketeer`, `Musketman`,
`Nuclear Submarine`, `Panzer`, `Paratrooper`, `Pikeman`, `Privateer`,
`Radar Artillery`, `Rider`, `Rifleman`, `Samurai`, `Scout`, `Settler`,
`Settler Industrial Ages`, `Settler Modern Times`, `Spearman`, `Stealth Bomber`,
`Stealth Fighter`, `Submarine`, `Swordsman`, `Tactical Nuke`, `Tank`,
`Transport`, `War Chariot`, `War Elephant`, `Worker`, `Worker Industrial Ages`,
`Worker Modern Times`, `chariot`, `warrior`.

Conquests adds art directories for expansion units and scenario content, with
some repeats/alternate names (for example `Crusader`, `Curragh`, `Dromon`,
`Trebuchet`, `Enkidu Warrior`, `Swiss Mercenary`, `Chasquis Scout`, `Javelin
Thrower`, `Ancient Cavalry`, `Hussar`, `Carrack`, `Combat Engineer`, and WWII
scenario units). Do not automatically add every such directory to the standard
campaign roster: verify the matching ruleset/BIQ and classify scenario-only
units separately.

## Art and animation coverage check

The three campaign art roots contain 156 unit folders and 156 `.ini` files:
76 base, 45 Play the World, and 35 Conquests. Each folder has exactly one
`.ini`, and every `.ini` has a nonempty `DEFAULT` entry. The 92 standard
campaign identities all map to an art folder using explicit name/era aliases
where needed, including Modern Paratrooper to the base Paratrooper art,
Numidian Mercenary to the Libyan Mercenary art, Hwach'a to Hwacha, and Chasqui
Scout to Chasquis Scout. Army and Leader use era-specific folders.

I resolved the animation paths declared by these `.ini` files with Windows
separator and case-insensitive handling, including references to shared files
in another art root. All referenced FLC animation files resolve somewhere in
the installed source tree. Some folders intentionally contain only an `.ini`
and sounds because their animation frames are shared from another unit folder;
the loader needs to normalize Windows `\\` separators, resolve relative paths,
and fall back across the base/PTW/Conquests unit roots instead of assuming each
animation is a local file. Scenario-pack `.ini` files are outside the
156-folder campaign count.

The loader in `src/client/tileset.rs` expands every nonempty action in
`[Animations]`, retains per-action `[Timing]`, resolves shared Windows-style
paths across the base/PTW/Conquests roots, and maps all 92 identities (98 INIs
including era-specific Army and Leader art). Army and Leader select the INI
for the owner's current research era. Every declared animation name has a
loader role, though gameplay/render triggers cover only actions currently used
by the simulation; `[Sound Effects]` and INI `[Speed]` settings are not
integrated. The source-tree coverage audit is in
[`CIV3_CONQUESTS_UNIT_ART_COVERAGE_AUDIT.md`](CIV3_CONQUESTS_UNIT_ART_COVERAGE_AUDIT.md).
The Conquests Paratrooper row now prefers its Conquests INI, which includes a
`BUILD` animation absent from the base Paratrooper manifest. A full source FLC
decoding check exists but has not yet been run as part of this rebuild audit.

## Canonical standard campaign roster (92 identities)

These names are deduplicated from the installed Conquests rules file. “Modern
Paratrooper” is the regular upgrade-chain unit; `Paratrooper` is a separate
WWII/Conquests rules entry. The Civ-specific duplicate prototypes do not add
new identities.

| Category | Civ III: Conquests identities |
| --- | --- |
| Civilian, reconnaissance, and special | Settler, Worker, Scout, Explorer, Leader, Army |
| Core land | Warrior, Archer, Spearman, Swordsman, Chariot, Horseman, Pikeman, Longbowman, Musketman, Knight, Rifleman, Cavalry, Infantry, Marine, Tank, Mech Infantry, Modern Armor |
| Artillery and missile | Catapult, Cannon, Artillery, Radar Artillery, Trebuchet, Cruise Missile, Tactical Nuke, ICBM |
| Naval | Galley, Curragh, Caravel, Dromon, Frigate, Man-O-War, Privateer, Galleon, Carrack, Ironclad, Transport, Carrier, Submarine, Destroyer, Battleship, AEGIS Cruiser, Nuclear Submarine, Cruiser |
| Air and airborne | Fighter, Bomber, Helicopter, Jet Fighter, Stealth Fighter, Stealth Bomber, F-15, Modern Paratrooper, Paratrooper |
| Unique, wonder, and expansion land units | Jaguar Warrior, Bowman, Hoplite, Impi, Legionary, Immortals, War Chariot, Rider, Mounted Warrior, Musketeer, Samurai, War Elephant, Cossack, Panzer, Keshik, Conquistador, Berserk, Sipahi, Gallic Swordsman, Ansar Warrior, Numidian Mercenary, Hwach'a, Medieval Infantry, Guerilla, Enkidu Warrior, Three-Man Chariot, Swiss Mercenary, Chasqui Scout, Javelin Thrower, TOW Infantry, Flak, Mobile SAM, Ancient Cavalry, Crusader |

The base campaign and Conquests additions are mixed in this table because the
target is the installed Conquests ruleset. Unique units retain their own
identity even when they replace a standard unit for one civilization.

## Catalog decisions

This table records the internal-name migration. `ALL_NATIVE` reflects the
source roster, and several legacy-shaped native variant names have now been
renamed to their Conquests identities. The non-campaign variants listed below
remain to be removed or separated from campaign runtime. `Storm`, `Howitzer`,
and `Dragoons` have been removed. Artillery now upgrades through its BIQ rule
to Radar Artillery; Military Tradition's unlock text names Cavalry.

### Source mappings present; internal identity cleanup remains

| Current entry | Civ III identity or treatment |
| --- | --- |
| Settler | Uses source identity `settler`; the internal variant is now singular. |
| Worker, Scout, Explorer, Warrior, Archer, Horseman, Chariot, Longbowman, Cavalry, Infantry, ModernArmor, Catapult, Cannon, Artillery, CruiseMissile, Icbm, Caravel, Frigate, Galleon, Ironclad, Transport, Carrier, Submarine, Destroyer, Battleship, AegisCruiser, NuclearSubmarine, Fighter, Bomber, Helicopter, JetFighter, StealthFighter, StealthBomber, Leader, Privateer | Source IDs and rules are present in the 92-row table. Rename internal identifiers to match BIQ identity where they still differ, then verify each behavior. |
| Pikemen, Riflemen, Marines, Paratroopers, MechInf | Renamed to Pikeman, Rifleman, Marine, Paratrooper, and MechInfantry. Modern Paratrooper remains a separate source identity. |
| Knights, Elephants | Renamed to Knight and WarElephant. |
| Armor | Renamed to Tank; `ModernArmor` remains a separate source identity. |
| Musketeers | Renamed to Musketman; the French unique Musketeer remains distinct. |
| Nuclear and TacticalNuke | Source table contains separate Tactical Nuke and ICBM identities. Remove the non-campaign `Nuclear` variant. |
| Partisan | Renamed to source identity `guerilla` (Guerilla); remove legacy behavior. |
| Trireme | Renamed to source identity `galley` (Galley). |
| Engineer | Removed as a unit identity and worker upgrade. Combat Engineer art/rules remain scenario-specific in the source tree. |
| Crusaders | Renamed to standard wonder-created source identity `crusader` (Crusader). |

### Add missing standard or Conquests types

The source rows and `ALL_NATIVE` now contain the roster additions listed
above, including Army, the unique units, Conquests additions, and both
wonder-created units. Finish replacing internal aliases with source identities
and audit that no similar legacy type contributes stats, abilities,
prerequisites, or upgrade behavior.

### Remove from the standard campaign unit catalog

These types are outside `ALL_NATIVE`; remove their production,
upgrade, AI, UI, sprite, and unit-specific gameplay paths; do not leave runtime
aliases or compatibility shims for them. `Migrants` has been removed from the
enum, fallback rules, city actions, and playtest UI:

- `Caravan` and `Freight` (Civ II trade units; enum, production, AI, UI,
  mission, save, transport-test, and city-route paths have been removed).
- `Diplomat` and `Spy` (non-campaign identities; movement, transport,
  production, unit-mission, AI, and unit-interface paths have been removed).

Removed non-campaign identities so far: `Migrants`, `Phalanx`, `Legion`,
`Storm`, `Howitzer`, `Dragoons`, `AWACS`, `AlpineTroops`, `Fanatics`, and
`Nuclear`, and `BarbarianLeader`. Removing BarbarianLeader also removed the
non-Civ-III ransom, leader-fleeing AI, special transport rescue, and uprising
spawn paths; uprising groups now consist of ordinary campaign unit identities.
The standard Civilopedia describes planting spies and conducting espionage
missions, but the standard BIQ does not define a buildable Spy PRTO. Espionage
must be implemented as a city-level mission system; the inherited unit-based
mission path has been removed, and the Civ III mission interface and costs
remain outstanding. Scenario Spy unit art is not evidence for adding Spy to
the standard campaign roster.

## Civilization-specific unique units

The catalog must carry civilization availability explicitly. These 31 unique
units are restricted to their listed civilization and replace the listed
standard unit. The BIQ availability mask is authoritative; the online unit
indexes below cross-check the names and replacement mapping.

| Expansion | Unique unit | Civilization | Replaces |
| --- | --- | --- | --- |
| Base | Bowman | Babylonian | Archer |
| Base | Cossack | Russian | Cavalry |
| Base | F-15 | American | Jet Fighter |
| Base | Hoplite | Greek | Spearman |
| Base | Impi | Zulu | Spearman |
| Base | Immortals | Persian | Swordsman |
| Base | Jaguar Warrior | Aztec | Warrior |
| Base | Legionary | Roman | Swordsman |
| Base | Man-O-War | English | Frigate |
| Base | Mounted Warrior | Iroquois | Horseman |
| Base | Musketeer | French | Musketman |
| Base | Panzer | German | Tank |
| Base | Rider | Chinese | Knight |
| Base | Samurai | Japanese | Knight |
| Base | War Chariot | Egyptian | Chariot |
| Base | War Elephant | Indian | Knight |
| Play the World | Ansar Warrior | Arabs | Knight |
| Play the World | Berserk | Vikings | Longbowman |
| Play the World | Conquistador | Spanish | Explorer |
| Play the World | Gallic Swordsman | Celts | Swordsman |
| Play the World | Hwach'a | Koreans | Cannon |
| Play the World | Keshik | Mongols | Knight |
| Play the World | Numidian Mercenary | Carthaginians | Spearman |
| Play the World | Sipahi | Ottomans | Cavalry |
| Conquests | Carrack | Portuguese | Caravel |
| Conquests | Chasqui Scout | Inca | Scout |
| Conquests | Dromon | Byzantines | Galley |
| Conquests | Enkidu Warrior | Sumerians | Warrior |
| Conquests | Javelin Thrower | Mayans | Archer |
| Conquests | Swiss Mercenary | Dutch | Pikeman |
| Conquests | Three-Man Chariot | Hittites | Chariot |

For every unit, not only unique units, store `available_to` from the BIQ.
Defaulting all normal units to every civilization would be wrong for restricted
starting units and unique replacements. Also store the replaced unit as
metadata for production UI and upgrade/replacement logic; a unique unit remains
its own identity with its own stats and art.

## Starting units by civilization

The standard Conquests rules define one Settler and one Worker as the default
starting units. The Expansionist trait adds one Scout; it does not replace the
Settler or Worker. The installed Conquests RACE records mark Americans, Arabs,
Hittites, Incans, Mongols, Portuguese, Russians, and Zulus as Expansionist. The
standard Scout availability mask omits the Incans because their unique
replacement is Chasqui Scout; the starting profile should therefore grant that
unit to the Incans. The manual explains the base starting-unit rule and the
Expansionist Scout bonus, while the installed Conquests BIQ supplies the target
start-unit types. The manual predates Conquests, so its older trait assignments
are not the target civ list.

Represent this as civilization start profiles, not fields on individual unit
definitions:

| Profile | Starting stack |
| --- | --- |
| Default civilization | Settler ×1, Worker ×1 |
| Expansionist civilization | Settler ×1, Worker ×1, Scout ×1 (Chasqui Scout for the Incans) |

Difficulty bonuses for AI and scenario-defined starts are separate setup
overrides. They should not be mixed into the normal civilization profile.
The [Conquests civilization table](https://civ3.vi.ki/civilizations) lists the
Conquests traits, and the [manual's starting-unit example](https://manualzz.com/doc/25583228/sid-meier-s-civilization-iii-video-game-user-manual) illustrates the
Settler/Worker start with the possible Expansionist Scout.

## Source-derived ruleset data now checked in

`data/rulesets/civ3_conquests/units.tsv` now contains 92 entries generated
from the installed Conquests BIQ: 59 standard campaign identities, 31 unique
units, and two wonder-created units. It records gameplay stats, prerequisites,
resources, upgrades, availability, unique replacements, raw BIQ flags, source
prototype indices, and original art folder/root mappings. `source_ai_profiles`
retains the AI-role variants in duplicate BIQ prototypes instead of pretending
those records are identical. `civilization_starts.tsv` contains all 31 playable
civilizations, default Settler/Worker stacks, and the eight Expansionist Scout
bonuses, including Chasqui Scout for the Inca.

The catalog provenance is documented in
`data/rulesets/civ3_conquests/README.md`. The extraction utility is no longer
distributed. The installed source BIQ SHA-256 at export time is
`1bd610ab1420217f7d64a9214b60c0bc7686e4781a0d6e66590171535495609c`.
The Rust campaign catalog consumes these rules for unit identity, stats,
availability, prerequisites, unique replacement, and art mapping. Remaining
work is to audit each field against its source, remove inherited enum/runtime
fallbacks, and finish behavior and animation-trigger coverage. The presence of
the data file alone does not establish full gameplay fidelity.

## Special-mode prototypes outside the target roster

The Conquests rules file also defines named leaders and Princess prototypes
with no ordinary playable-civilization availability. They belong to Regicide
or Capture the Princess modes, outside the 92-type standard campaign roster.
Crusader and Ancient Cavalry also lack ordinary availability masks, but are
standard wonder-created units and are included above.

## Rebuild order and completion criteria

1. **Source map — populated; verification remains.** The 92-unit table records
   BIQ definitions, availability, replacement relationships, Civilopedia
   references, art `.ini` mappings, and declared animations. Start profiles
   record the default and Expansionist sets separately from scenario/difficulty
   overrides.
2. **Rules data and active roster — implemented; source audit remains.** The
   editable table feeds the 92-entry campaign catalog, including stats,
   technology, resources, upgrades, flags, class, cargo, and availability.
3. **Legacy removal — unit-specific cleanup implemented; shared behavior audit
   remains.** Removed the inherited Diplomat, Spy, Caravan, and Freight
   identities and their unit-only runtime paths, along with the previous
   per-unit stat/metadata fallbacks. Review shared simulation mechanics for
   remaining Freeciv assumptions and migrate or remove those that conflict
   with Civ III.
4. **Art loader — mapped; runtime coverage and verification remain.** Load each
   unit's original `.ini`, resolve shared paths across source art roots, and
   connect every in-scope animation action and timing to game states.
5. **Completion audit — outstanding.** The active game must have exactly the
   intended Civ III campaign inventory, source-backed rules and civilization
   restrictions, original art/animations, and no Freeciv-only unit or
   compatibility shim in runtime code. Scenario-only content stays outside
   the campaign catalog. Record unresolved source conflicts rather than
   guessing.
