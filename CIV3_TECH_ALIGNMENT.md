# Civ III technology alignment

Status: runtime research choices and progression now follow the Civ III base-game tree.
This file records the mapping and remaining content/unlock gaps. The target is
the original four-era tree; Conquests-only advances are listed separately.

## Reference target

Civ III's base game has 82 named advances: Ancient Times (21), Middle Ages
(22), Industrial Age (22), and Modern Times (17), followed by repeatable
Future Technology. A new era becomes available once all required advances in
the current era are known. Optional advances can be skipped and researched
later. Conquests adds Fascism and Ironclads and changes some prerequisite and
optional-advance details. Sources: [CivFanatics tech overview](https://civfanatics.com/civ3/infocenter/),
[Civ III Complete manual](https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/3900/manuals/manual.pdf?t=1724341287),
and [era-by-era tech reference](https://civ3.vi.ki/technologies).

The four eras also match the four architectural rows in the Civ III city
sprites. City artwork should therefore use the player's unlocked research era
directly. City-size art is a separate choice: Civ III's town/city/metropolis
population bands are 1–6, 7–12, and 13+ (Aqueduct and Hospital permit growth
past 6 and 12 respectively).

## Era contents and gates

| Era | Advance count | Advances optional in base game | Era-entry advances |
|---|---:|---|---|
| Ancient Times | 21 | Horseback Riding, Literature, Monarchy, The Republic | Starting era |
| Middle Ages | 22 | Chivalry, Democracy, Economics, Free Artistry, Military Tradition, Music Theory, Navigation, Printing Press | Engineering, Feudalism, Monotheism |
| Industrial Age | 22 | Advanced Flight, Amphibious Warfare, Communism, Espionage, Nationalism, Sanitation | Medicine, Nationalism, Steam Power |
| Modern Times | 17 | None | Computers, Ecology, Fission, Rocketry |

The era-entry advances are the first technologies available after the previous
era gate is satisfied; they are not a single transition technology. Modern
Times has no optional advances. In Conquests, Fascism and Ironclads are added
to the Industrial era; both are optional there.

### Canonical base-game catalog

**Ancient Times:** Alphabet, Bronze Working, Ceremonial Burial, Code of Laws,
Construction, Currency, Horseback Riding, Iron Working, Literature, Map
Making, Masonry, Mathematics, Monarchy, Mysticism, Philosophy, Polytheism,
Pottery, The Republic, Warrior Code, The Wheel, Writing.

**Middle Ages:** Astronomy, Banking, Chemistry, Chivalry, Democracy, Economics,
Education, Engineering, Feudalism, Free Artistry, Gunpowder, Invention,
Magnetism, Metallurgy, Military Tradition, Monotheism, Music Theory,
Navigation, Physics, Printing Press, Theology, Theory of Gravity.

**Industrial Age:** Advanced Flight, Amphibious Warfare, Atomic Theory,
Combustion, Communism, The Corporation, Electricity, Electronics, Espionage,
Flight, Industrialization, Mass Production, Medicine, Motorized Transportation,
Nationalism, Radio, Refining, Replaceable Parts, Sanitation, Scientific Method,
Steam Power, Steel.

**Modern Times:** Computers, Ecology, Fission, Genetics, Integrated Defense,
The Laser, Miniaturization, Nuclear Power, Recycling, Robotics, Rocketry,
Satellites, Smart Weapons, Space Flight, Stealth, Superconductor, Synthetic
Fibers.

## Current CivCraft catalog comparison

CivCraft retains prior serialized technology IDs and appends the missing
canonical nodes, so storage still has 99 entries. Only the 82 named Civ III
advances plus repeatable Future Technology are now researchable and shown in
the tree's four era tabs. The 17 legacy advances are hidden from research and
map to their closest canonical advance for existing gameplay effects and old
saves. Canonical prerequisites use the base-game dependency graph; required
advances gate each subsequent era, and optional advances remain researchable
later. Research costs and some AI valuation remain CivCraft rules rather than
an exact reproduction of Civ III balance.

Because Future and UNSET are derived from catalog length, the save format was
bumped. V16 saves remap the old sentinels, convert research in progress to the
canonical equivalent when available, and translate known legacy advances to
canonical advances.

Several existing nodes were renamed in place to their closest Civ III counterparts:

| Civ III base-game advance | Current CivCraft counterpart | Alignment note |
|---|---|---|
| Education | University (tech) | Renamed in place; serialized ID retained. |
| Nationalism | Conscription | Renamed in place; serialized ID retained. |
| Motorized Transportation | Mobile Warfare | Renamed in place; the old Automobile ID maps to this research choice. |
| Ecology | Environmentalism | Renamed in place; serialized ID retained. |
| Fission | Nuclear Fission | Renamed in place; serialized ID retained. |
| The Laser | Laser | Renamed in place; serialized ID retained. |
| Superconductor | Superconductors | Renamed in place; serialized ID retained. |
| Synthetic Fibers | Plastics | Renamed in place; serialized ID retained. |

The following base-game advances were absent and have now been added as
dedicated nodes: **Free Artistry, Genetics, Integrated Defense, Literature,
Military Tradition, Music Theory, Printing Press, Replaceable Parts, Satellites,
Scientific Method, and Steam Power**.

The 17 retained legacy IDs are **Automobile, Bridge Building, Combined Arms,
Explosives, Fusion Power, Genetic Engineering, Guerilla Warfare, Labor Union,
Leadership, Literacy, Machine Tools, Railroad, Refrigeration, Seafaring, Steam
Engine, Tactics, and Trade**. They are no longer independent research choices.
Their CivCraft effects are attached to the closest canonical advance (for
example Railroad and Steam Engine effects follow Steam Power); old saves migrate
known and in-progress legacy advances. These analogies preserve existing
systems but do not mean Civ III had the same mechanics.

## Unlock coverage gaps to resolve

The runtime catalog has 57 Civ2Civ3 unit types. Existing analogs cover most of
the familiar progression, but a catalog entry with a similar name does not
guarantee matching Civ III unlock, resource prerequisite, upgrade path, or
special rule. The base-game reference has 46 units, including combat units,
workers, settlers, Army/Leader systems, naval units, and aircraft.

### Units

| Civ III base-game unit or family | CivCraft state | Gap |
|---|---|---|
| Warrior, Settler, Worker, Archer, Horseman, Chariot, Catapult, Swordsman, Pikeman, Knight, Musketman, Cannon, Cavalry, Rifleman, Artillery, Tank, Marine, Paratrooper, Mech Infantry | Direct or close analogs exist | Techs are mapped for most; compare every unlock and strategic resource requirement. Current unit catalog and tech unlock metadata are still not a full Civ III rules port. |
| Galley, Caravel, Galleon, Frigate, Ironclad, Destroyer, Transport, Submarine, Battleship, Carrier, AEGIS Cruiser | Trireme, Caravel, Galleon, Frigate, Ironclad, Destroyer, Transport, Submarine, Battleship, Carrier, AEGIS Cruiser | Most have direct/class analogs and mapped techs; verify resource requirements, stats, sea-loss, cargo, stealth, and upgrade paths. Galley is separately noted below. |
| Fighter, Bomber, Helicopter, Cruise Missile, Stealth Fighter, Stealth Bomber | Matching or close CivCraft types | Techs align for direct analogs; air missions, resource costs, and combat rules still differ. |
| Spearman | Phalanx is the Bronze Working defender | Partial class analog; stats, name, and upgrade progression differ. |
| Scout | No dedicated Scout | Missing; Civ III Scout is an Expansionist starting-unit exception, not an ordinary tech unlock. |
| Longbowman | No distinct type | Missing Invention unit and bombard role. |
| Infantry | No distinct type | Missing Replaceable Parts unit tier. |
| Modern Armor | No distinct type; Armor is the Tank analog | Missing Synthetic Fibers upgrade and Aluminum requirement. |
| Radar Artillery | Howitzer is only a rough analog | Missing Robotics unlock, bombard profile, and Aluminum requirement. |
| Privateer | No distinct type | Missing Magnetism unlock, neutral nationality/privateer rules, and Iron/Saltpeter requirements. |
| Jet Fighter | No distinct type | Missing Rocketry unlock and air missions. |
| Nuclear Submarine | No distinct type | Missing Fission unlock, stealth, Uranium requirement, and Tactical Nuke carrying. |
| ICBM and Tactical Nuke | One generic Nuclear type exists | Missing separate types and strategic-resource requirements; the source lists ICBM at Satellites and Tactical Nuke at Space Flight. |
| Army | No Civ III-style stack-of-units Army | Missing Army formation, capacity, movement/combat behavior, and Pentagon/Military Academy interactions. |
| Leader | A generic Leader unit exists | Partial only; verify generation from elite victories and Army creation/upgrade rules. |
| Civ-specific unique units (Hoplite, Impi, Immortal, Legionary, Mounted Warrior, Samurai, War Chariot, Rider, Cossack, Panzer, Bowman) | No civilization-specific unit replacement system | Missing conditional replacements. Several existing units have the same or related names but are not civilization-gated. |
| Galley | Trireme is a close early-ship analog | Partial; Civ III unlocks Galley at Map Making and has sea-loss/cargo rules that need comparison. |

Base-game unit unlocks and resources are listed in the [Civ III unit
reference](https://civ3.vi.ki/units). Expansion-only units such as the
Conquests Man-O-War are outside this base-game gap count.

### Worker, resource, map, and diplomacy effects

| Civ III technology effect | CivCraft mapping | Status |
|---|---|---|
| Construction: build Fortresses | Construction enables Fort/Fortress work | Supported, though CivCraft also has a separate Fort tier. |
| Engineering: plant forests and cross rivers with roads | Engineering gates Plant work and river road/railroad work | Supported for these actions. |
| Flight: build Airfields | Flight gates Airstrip/Airbase work | Supported; CivCraft's two-tier bases are a local extension. |
| Electricity: irrigate without a nearby water/river source | Electricity counts as an irrigation source | Supported. |
| Steam Power: build Railroads | Steam Power gates Railroad work | Supported; river crossing additionally requires Engineering. |
| Replaceable Parts: faster workers | Replaceable Parts doubles worker activity rate | Supported for active work and worker projections. |
| Strategic resources become visible at their reveal advances | Horses: The Wheel; Iron: Iron Working; Saltpeter: Gunpowder; Coal: Steam Power; Oil: Refining; Rubber: Replaceable Parts; Aluminum: Rocketry; Uranium: Fission | Supported by resource reveal mapping; resource consumption/requirements on units remain a separate audit. |
| Strategic resource requirements for units | Resources are visible on tiles, but unit metadata/production has no Horses, Iron, Saltpeter, Coal, Oil, Rubber, Aluminum, or Uranium prerequisite checks | Missing across the unit roster; Civ III unit unlocks alone do not permit construction without required connected resources. |
| Masonry: build Outposts | No Outpost base type or action | Missing. |
| Satellites: reveal the world map | Satellites has no global map-reveal effect | Missing. Internet currently reveals cities, which is a different effect. |
| Writing: embassies, Right of Passage, and Military Alliances | Embassies and treaties are available through current diplomacy rules | Tech gates are missing; capabilities are not unlocked by Writing. |
| Navigation: trade maps | World/sea map trade exists in treaties | Navigation gate is missing. |
| Printing Press: communications trading | No communications-trade system | Missing. |
| Nationalism: Mutual Protection Pacts, embargoes, mobilization, and drafting | No corresponding MPP/embargo/mobilization system; conscription is a government/unit mechanic | Mostly missing; conscription is only a partial thematic analogue. |

The listed Civ III effects are based on the base-game advance reference at
[Civ III technologies](https://civ3.vi.ki/technologies). The diplomacy table
describes tech unlocks, not a claim that all Civ III treaty mechanics are
otherwise implemented here.

### Buildings, wonders, and projects

The base-game reference has 24 Great Wonders and 10 Small Wonders. CivCraft's
catalog has many name/effect analogs, but stores them in Civ2Civ3 categories;
those categories do not always match Civ III's Great versus Small Wonder
classification, and several global uniqueness, continent, resource, and
obsolescence rules differ.

The 19 Great Wonder analogs present are Colossus, Great Library, Lighthouse,
Great Wall, Hanging Gardens, Pyramids, Copernicus' Observatory, J.S. Bach's
Cathedral, Leonardo's Workshop, Magellan's Expedition, Michelangelo's Chapel,
Isaac Newton's College, Shakespeare's Theater, A.Smith's Trading Company, Sun
Tzu's War Academy, Hoover Dam, Cure for Cancer, Manhattan Project, and United
Nations. Five base-game Great Wonders have no catalog entry: Oracle
(Mysticism), Theory of Evolution (Scientific Method), Universal Suffrage
(Industrialization), SETI Program (Computers), and Longevity (Genetics). The
tech reference and Prima wonder table disagree on the United Nations
prerequisite (Fission vs Radio); the current catalog uses Fission. CivCraft
also has Conquests-only
Statue of Zeus, Mausoleum of Mausollos, and Temple of Artemis, but does not
model Zeus's Ivory condition. Internet is Play the World content.

| Civ III item | CivCraft state | Gap |
|---|---|---|
| Colosseum | Amphitheater is a close happiness-building analog | Partial: distinct item/prerequisite and exact happiness behavior are not represented. |
| Hospital | No direct item; Sewer System exists | Missing growth cap and hospital effects. |
| Longevity | No direct item | Missing Great Wonder (Genetics). |
| Oracle, Theory of Evolution, Universal Suffrage, SETI Program | No direct item | Missing four base-game Great Wonders and effects. |
| Forbidden Palace, Heroic Epic, Iron Works, Military Academy, Pentagon, Wall Street, Intelligence Agency, Battlefield Medicine | No direct item | Missing eight base-game Small Wonders and their effects/conditions. Apollo Program and SDI Defense exist but are also Small Wonders in Civ III; local catalog labels them Great Wonders. |
| Statue of Zeus, Mausoleum of Mausollos, Temple of Artemis | Present; Conquests-only | Techs mapped to Mathematics, Philosophy, and Polytheism; Zeus Ivory requirement unsupported. |
| Manhattan Project | Present; Fission unlock | Uranium prerequisite is not modeled. |
| Knights Templar | No direct item | Missing Conquests wonder and Crusader spawning effect; outside base-game scope. |
| Internet | Present; Play the World expansion | Expansion content, not in the 2001 base-game target. |
| Eiffel Tower, Darwin's Voyage, Marco Polo's Embassy, Statue of Liberty, Women's Suffrage, King Richard's Crusade | Present in CivCraft as legacy/Civ II style catalog entries | CivCraft-only catalog content; not substitutes for missing Civ III wonders. |

Names such as University/Isaac Newton's College and A.Smith's Trading
Company/Smith's Trading Company are likely aliases, not missing content. Their
tech unlocks and effects still need direct checks against the Civ III source.
Matching city-improvement unlocks have been corrected for Airport (Flight),
Aqueduct (Construction), Bank (Banking), Cathedral (Monotheism), City Walls
(Masonry), Courthouse (Code of Laws), Factory (Industrialization), Granary
(Pottery), Harbor (Map Making), Hydro Plant (Electronics), Library (Literature),
Marketplace (Currency), Mass Transit (Ecology), Manufacturing Plant (Robotics),
Nuclear Plant (Nuclear Power), Offshore Platform (Miniaturization), Police
Station (Communism), Power Plant/Coal Plant (Industrialization), Recycling
Center (Recycling), Research Lab (Computers), SAM Battery (Rocketry), SDI
Defense (Integrated Defense), Sewer System (Sanitation), Solar Plant (Ecology),
Stock Exchange (Economics), Supermarket (Sanitation), Temple (Ceremonial Burial),
and University (Education). Coastal Defense is mapped to Metallurgy as the
closest Coastal Fortress role. The remaining improvement-level differences,
including Colosseum and Hospital, and all building effects, costs, size limits,
resource conditions, and obsolescence rules still need source-by-source
comparison. See the gap table above for the complete currently identified
missing Civ III Great and Small Wonder entries.

### Governments

The current model contains **Anarchy, Despotism, Monarchy, Communism, Republic,
Democracy, Tribal, Fundamentalism, and Federation**. Civ III's base game has
five selectable forms plus transitional Anarchy: Despotism, Monarchy,
Communism, Republic, and Democracy. Tribal, Fundamentalism, and Federation are
CivCraft-specific and should remain only if intentionally retained as
extensions. Despotism is the starting form in both games.

| Civ III government | Unlock | CivCraft status |
|---|---|---|
| Despotism | Starting government | Present; runtime start. The despotism tile penalty and full support/rush rules need comparison. |
| Monarchy | Monarchy | Present; research path matches, government effects need comparison. |
| The Republic | The Republic | Present; path matches, war weariness, commerce, support, and rush behavior need comparison. |
| Communism | Communism (Nationalism is its prerequisite) | Present; unlock maps through the canonical Communism tech, but corruption, police, espionage, support, and rush effects need comparison. |
| Democracy | Democracy (Printing Press and Banking are prerequisites) | Present; current requirement maps to canonical Democracy, but worker bonus, war weariness, propaganda immunity, commerce, and corruption need comparison. |
| Anarchy | Transitional state during revolution | Present as a government state; revolution duration, zeroed economy, and turn handling need comparison. |

The extra Tribal, Fundamentalism, and Federation forms and their tech paths
are CivCraft extensions. Civ III comparison also needs the per-government
corruption/waste model, military police, unit support, population/gold rush,
war weariness, draft limits, and Democracy worker-speed bonus. Civ III
Conquests adds Feudalism and Fascism to the standard government set; these
are outside the base-game target. Secret Police HQ is also Conquests content.

## Implementation constraints

- Tech identifiers are serialized and referenced throughout unit, building,
  government, city-art, AI, and UI code. Preserve existing IDs where possible;
  if catalog indices or `Future` move, add an explicit save migration.
- Research availability now supports normal prerequisites and era entry gated
  by required canonical advances. Optional advances remain researchable later.
- AI target/path selection now traverses missing required advances at era gates;
  trading, huts/free-tech discovery, and Future Tech use canonical eligibility.
- The research view now shows the four named era tabs, marks era-entry advances,
  and labels Future Tech's completion requirement.
- Unlock audit should be maintained as a separate machine-readable or
  hand-checkable inventory with source, Civ III unlock, CivCraft analog, and
  status (supported, partial, missing, or intentionally different).

## Source and version notes

This document uses the **base-game 82-advance catalog** for the canonical list
and cites Conquests separately because its Industrial-era additions and some
prerequisite changes are material. The local Freeciv Civ2Civ3 ruleset is the
source for the current CivCraft catalog, not an authoritative substitute for
Civ III rules. The research graph uses base game; unlock metadata and effects
still need a source-by-source pass before the conversion is gameplay-complete.
