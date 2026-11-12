# Civ III installation sprite path comparison: OpenCiv3 and CivCraft

## Purpose and path conventions

This document maps sprite files requested by OpenCiv3 and CivCraft back to the official Civilization III installation. It focuses on which installation paths are read and how the files are interpreted, cropped, or selected. It does not specify a generic asset-loader port.

In the tables, `<INSTALL>` is the folder containing `civ3id.mb` and the `Art/`, `civ3PTW/`, and `Conquests/` folders. OpenCiv3 usually requests a logical path beginning `Art/...`; its `Civ3MediaPath` checks that relative path under `<INSTALL>/Conquests/`, then `<INSTALL>/civ3PTW/`, then `<INSTALL>/`. Thus `Art/Terrain/xtgc.pcx` resolves first to `<INSTALL>/Conquests/Art/Terrain/xtgc.pcx`, then PTW, then base if earlier candidates are absent. Mod/scenario paths and OpenCiv3 packaged assets can take precedence. CivCraft's default `civ3`/`conquests` theme starts from `<INSTALL>/Conquests/Art`; its fallback to PTW and base is presently conditional on the install root directory being named exactly `source`.

CivCraft's `Theme::image` asks for a `.png` name first and also tries the matching lowercase `.pcx` extension. The official installation normally supplies PCX. For example, it constructs `Terrain/XTGC.pcx` from the PNG request. Several official filenames are lowercase (`xtgc.pcx`), and the image reader does not perform case-insensitive resolution; those case-only differences can therefore make the file fail to load on a case-sensitive filesystem. OpenCiv3's `Civ3MediaPath` does resolve each component case-insensitively on non-Windows platforms.

Sources inspected: `openciv3/C7/Util.cs`, `openciv3/C7/Lua/civ3/textures/terrain.lua`, `terrain_improvements.lua`, `cities.lua`, `resources.lua`, `unit_icons.lua`, `civ_colors.lua`, `openciv3/C7/Animations/AnimationManager.cs`, `app/src/client/tileset.rs`, and `app/data/rulesets/civ3_conquests/units.tsv`.

## World terrain and map sprites

| Sprite role | OpenCiv3 logical path(s) | CivCraft logical name / actual install candidate | How the sprite is used; match |
| --- | --- | --- | --- |
| Land transition sheets | `Art/Terrain/xtgc.pcx`, `xpgc.pcx`, `xdgc.pcx`, `xdpc.pcx`, `xdgp.pcx`, `xggc.pcx` | `Terrain/XTGC`, `XPGC`, `XDGC`, `XDPC`, `XDGP`, `XGGC` (`.png` then `.pcx`) under Conquests Art, then conditional PTW/base fallback | Both select 128×64 tiles from the same 9×9 transition grids. Sheet IDs differ in internal ordering: OpenCiv's Lua lists `xtgc,xpgc,xdgc,xdpc,xdgp,xggc`; CivCraft has an explicit lookup table. On case-sensitive filesystems the uppercase CivCraft request does not match lowercase official filenames. Verify ID-to-sheet mapping and path case. |
| Water transition sheets | `Art/Terrain/wCSO.pcx`, `wSSS.pcx`, `wOOO.pcx` | `Terrain/wCSO`, `wSSS`, `wOOO` | Both use the official 1152×576 sheets and tile crops. |
| Snow/polar cap | No separate base entry in the OpenCiv3 `terrain.base` sheet mapper shown here; terrain overlays include `Mountains-snow.pcx` | `Terrain/polarICEcaps-final` | CivCraft previously drew a keyed cap from this sheet. The renderer no longer loads or draws it; polar boundaries remain ocean. |
| Mountain overlay | `Art/Terrain/Mountains.pcx`, `mountain jungles.pcx`, `Mountains-snow.pcx`, `mountain forests.pcx` | `Terrain/Mountains`, plus CivCraft's generic vegetation sheet; no separate snow/jungle/mountain-forest role mapping in the same overlay table | OpenCiv selects a mountain variant by terrain extras. CivCraft maps a single mountain sheet and has optional overlay assets; compare actual rendering coverage as well as source file. |
| Hills overlay | `Art/Terrain/xhills.pcx`, `hill forests.pcx`, `hill jungle.pcx` | `Terrain/XHILLS` | CivCraft maps the base hills overlay. Its uppercase `XHILLS` request does not match installed lowercase `xhills.pcx` on a case-sensitive filesystem. Separate forest/jungle hill variants are not separately named in the current `OVERLAYS` table. |
| Volcano | `Art/Terrain/Volcanos.pcx`, `Volcanos forests.pcx`, `Volcanos jungles.pcx` | `Terrain/Volcanos` | CivCraft maps the base volcano sheet; OpenCiv can choose forest/jungle variants. |
| Marsh | `Art/Terrain/marsh.pcx` | `Terrain/marsh` | Both crop multiple large/small marsh shapes from this sheet; source coordinates/layout should be compared. |
| Forest/jungle/pine vegetation | `grassland forests.pcx`, `plains forests.pcx`, `tundra forests.pcx` | `Terrain/grassland forests` is the single required vegetation atlas | OpenCiv has terrain-specific atlases for grassland, plains, and tundra, plus separate jungle and pine rows. CivCraft currently uses the grassland atlas for its vegetation role; this is a visible source-selection mismatch for plains/tundra forests unless the renderer derives them by another path. |
| Flood plains | `Art/Terrain/floodplains.pcx` is an installed Conquests addition; not a named base mapper entry in inspected OpenCiv Lua | `Terrain/floodplains` optional overlay | CivCraft supports this Conquests sheet optionally. |
| Rivers | `Art/Terrain/mtnRivers.pcx`, `deltaRivers.pcx` | `Terrain/mtnRivers`, `deltaRivers` | Both use the official mountain and delta river sheets. CivCraft requires both. |
| Territory borders | `Art/Terrain/Territory.pcx` | `Terrain/Territory.pcx` | Same source. OpenCiv samples 128×72 cells from a 2-column×4-row sheet; CivCraft recolors keyed pixels and uses its own intersection layout. |
| Terrain buildings | `Art/Terrain/TerrainBuildings.pcx` | `Terrain/TerrainBuildings` | Both crop from the official sheet. OpenCiv maps fortress, mine, barricade, and barbarian camp at explicit cells; CivCraft's current helper maps fortress (0,0), barbarian camp (2,0), and mine (2,1). OpenCiv's improvement table also maps barricade (3,0); compare supported roles. The installation uses `TerrainBuildings.PCX`, so CivCraft's lowercase extension candidate also fails on case-sensitive filesystems. |
| Irrigation | `Art/Terrain/irrigation.pcx`, `irrigation PLAINS.pcx`, `irrigation DESETT.pcx`, `irrigation TUNDRA.pcx` | `Terrain/irrigation`, `irrigation PLAINS`, `irrigation DESETT`, `irrigation TUNDRA` | Same four official files (including the source spelling `DESETT`). CivCraft treats them as optional. |
| Roads/railroads | `Art/Terrain/roads.pcx`, `railroads.pcx` | `Terrain/roads`, `railroads` | Same sheets. OpenCiv uses PCX directly; CivCraft tries PNG then PCX. |
| Pollution/craters | `Art/Terrain/pollution.pcx`, `craters.pcx` | `Terrain/pollution`, `craters` | Same official files; CivCraft treats them as optional overlays. |
| Flood plains and advanced improvements | Conquests/Art/Terrain includes other files, but no matching entries in the inspected OpenCiv base improvement mapper | `Terrain/floodplains`, `farmland`, `oil-well`, `oil-platform`, `airstrip`, `airbase` | CivCraft's optional set includes additional Conquests/tileset roles. Resolve the actual installed filename and crop before claiming a match. |
| Fog alpha | `Art/Terrain/FogOfWar.pcx` | Direct `<INSTALL>/Art/Terrain/FogOfWar.pcx` | Same logical source file, but different root behavior: OpenCiv's resolver gives Conquests/PTW copies precedence; CivCraft bypasses its theme and reads base `Art` directly. |
| Yield overlay | `Art/Terrain/tnt.pcx` | No matching official source sheet is loaded by the CivCraft tileset (yield/minimap UI is drawn in code) | OpenCiv uses the installation's TnT sheet for yield visualization. |
| Resource icons | `Art/resources.pcx`; small luxuries: `Art/city screen/luxuryicons_small.pcx` | CivCraft optionally loads `resources.png` and `resources_shadows.png` from the selected theme root | CivCraft has no direct `.pcx` fallback names corresponding to those two defaults; confirm whether PNG conversions are present in the selected art directory. OpenCiv crops 50×50 large icons and 22×22 small luxury icons. |

## Cities, units, and other entity sprites

| Sprite role | OpenCiv3 logical path(s) | CivCraft logical name / actual install candidate | How the sprite is used; match |
| --- | --- | --- | --- |
| City buildings, middle eastern default | `Art/Cities/rMIDEAST.pcx`, `Art/Cities/MIDEASTWALL.pcx` | `Cities/rMIDEAST.png` is among the five required style sheets; `.pcx` candidate is available | OpenCiv's inspected Lua currently uses only Middle Eastern buildings for all cities, selecting 166×95 cells by size rank and era. CivCraft selects among five styles and loads 501×380 sheets; this is a different crop/layout, not a path-equivalent implementation. |
| City buildings, full style set | OpenCiv3 asset/data paths include `Art/Cities/rAMER.pcx`, `rASIAN.pcx`, `rEURO.pcx`, `rMIDEAST.pcx`, `rROMAN.pcx` and corresponding `AMERWALL.PCX`, `ASIANWALL.PCX`, `EUROWALL.PCX`, `MIDEASTWALL.PCX`, `ROMANWALL.PCX`; however the inspected `textures/cities.lua` currently binds only the Middle Eastern pair | `Cities/rAMER`, `rASIAN`, `rEURO`, `rMIDEAST`, `rROMAN` | Official installation uses uppercase `.PCX` extensions for these files. CivCraft tries lowercase `.pcx`, so the paths fail on case-sensitive filesystems. Its current loader also does not separately load the wall sheets. Confirm wall rendering before calling this complete parity. |
| Unit map animation | `Art/Units/<PediaIcons art name>/<same art name>.INI`; each action's `.flc` is named by that INI. | TSV `art_root/art_folder` selects an edition-specific INI directory; role and era variants are loaded from its INI, then FLC references are resolved relative to that INI and same-unit fallback folders | Both are INI-driven and use official FLCs. OpenCiv asks the shared resolver for `Art/Units/...` (Conquests → PTW → base); CivCraft maps each roster identity to an explicit edition folder, with era-specific folder variants for Leader/Army. Full CivCraft campaign mapping is tabulated below. |
| Unit portrait/icon | OpenCiv's classic ruleset uses `Art/Units/units_32.pcx`; crop index is based on the imported Civilopedia icon/thumbnail metadata and era variation | CivCraft draws the first frame of the selected unit FLC as the compact portrait; it does not load `units_32.pcx` | Both resolve to unit identity, but the sprite source differs: OpenCiv uses the official menu/icon atlas while CivCraft reuses animation art. |
| Unit palette/player color | `Art/Units/Palettes/ntp00.pcx` through `ntp31.pcx` are referenced by OpenCiv's classic color texture mapping | CivCraft uses hard-coded RGB source-color mappings in `sprite_metadata.rs` and derives tint from FLC palette indices; it does not load `ntp00..ntp31.pcx` as separate files | Different source mechanism. Verify color index equivalence per civilization; matching a unit FLC alone does not guarantee matching team-color output. |
| Cursor animation | `Art/Animations/Cursor/Cursor.flc` | Direct `<INSTALL>/Art/Animations/Cursor/Cursor.flc` | Same official base path and FLC. OpenCiv obtains it through generic media path resolution; CivCraft reads base path directly. |
| Animated effect INIs | `Art/Animations/<effect category>/<ini>`; inspected OpenCiv categories include `hit.ini`, `hit2.ini`, `hit3.ini`, `hit5.ini`, `miss.ini`, and `water miss.ini` under the configured effect folders | No equivalent generic effects-FLC set is currently loaded in CivCraft's client tileset | This is a feature-coverage difference; don't confuse with unit FLC parity. |
| Citizen heads | OpenCiv classic sprite config uses `Art/SmallHeads/popHeads.pcx` | CivCraft draws citizen/population UI without this installed sheet | No direct shared sprite path in current CivCraft rendering. |
| Ruins/goody huts | OpenCiv improvement map uses `Art/Cities/DESTROY.PCX` for ruins; terrain folder includes `goodyhuts.pcx` in the base install | CivCraft does not load `Cities/DESTROY.PCX` as a required atlas in its current tileset table | Conquests `DESTROY.PCX` also exists under `Conquests/Art/Cities`; OpenCiv resolver finds that first. |

## Interface and screen sprite paths

OpenCiv3's classic Lua texture configuration names additional original interface atlases. CivCraft currently draws most controls and screens in Rust and does not request matching official PCX sheets from its client asset module; these are intentional coverage differences unless original-UI parity is a requirement.

| OpenCiv3 logical install path | Sprite role | CivCraft counterpart |
| --- | --- | --- |
| `Art/buttonsFINAL.pcx`, `Art/3checkboxes-USE.pcx`, `Art/exitBox-backgroundStates.pcx`, `Art/X-o_ALLstates-sprite.pcx` | Main button, checkbox, exit-dialog, and X/O state atlases | Custom Rust-drawn buttons and indicators; no matching installation sprite request found. |
| `Art/interface/` (including configured button and rollover sheets), `Art/interface/MovementLED.PCX` | Interface controls and unit movement LEDs | Custom HUD controls; movement LEDs are the one matching asset path loaded by CivCraft: `interface/MovementLED.PCX` from the selected theme, falling back to Conquests Art. |
| `Art/TileInfo.pcx`, `Art/popupborders.pcx`, `Art/Cursor.pcx` | Tile information, popup frame, classic cursor sprite | Custom panels/cursor presentation; the installed animated cursor separately is `Art/Animations/Cursor/Cursor.flc`, which CivCraft does load. |
| `Art/Tech Chooser/scienceNAV.pcx`, `Art/Advisors/science_ancient.pcx`, `science_middle.pcx`, `science_industrial_new.pcx`, `science_modern.pcx` | Science advisor backgrounds, buttons, and navigation | CivCraft draws the research tree and related UI with native Rust geometry and text; no source atlas lookup. |
| `Art/Advisors/dialogbox.pcx`, `military.pcx`, `domestic.pcx`, `domesticBUTTON.pcx`, `domestic_plusminus.pcx`, `trade.pcx`, `foreign.pcx`, `foreignTAB.pcx`, `culture.pcx`, `wonders_background.pcx`, `demographics.pcx` | Advisor panels and controls | No corresponding original advisor screens are loaded by the current client. |
| `Art/SmallHeads/popupDOMESTIC.pcx`, `popupTRADE.pcx`, `popupMILITARY.pcx`, `popupFOREIGN.pcx`, `popupCULTURE.pcx`, `popupSCIENCE.pcx` | Advisor popup portraits | No matching official portrait atlas lookup in CivCraft. |
| `Art/WorldSetup/` (e.g. `age.pcx`, `climate.pcx`, `temperature.pcx`, `CLIMTEMPAGEDepress.pcx`, `CLIMTEMPAGERollovers.pcx`) | World setup controls and rollovers | CivCraft's world setup uses custom UI; no matching installation sprite lookup found. |
| `Art/city screen/cityMgmtButtons.pcx`, `ProdButton.pcx`, `CityIcons.pcx`, `background.pcx`, `buildings-small.pcx`, `buildings-large.pcx`, `ProductionQueueBox.pcx`, `luxuryicons_small.pcx` | City screen controls, icons, building buttons, and luxury icons | CivCraft's current city screens are custom/partial. The `luxuryicons_small.pcx` path is also used by OpenCiv's resource-small mapper; CivCraft does not presently load it. |
| `Art/Cities/city icons.pcx`, `Art/Units/units_32.pcx`, `Art/Units/Palettes/ntp00.pcx`–`ntp31.pcx` | City/strategic unit icons and palette colors | CivCraft draws unit icons from animation FLC frames and computes colors in code; no installed icon atlas or palette file is loaded. |
| `Art/PalaceView/bkgr.pcx`, `Art/SpaceShip/SHIPbackground.pcx`, `Art/Credits/` | Palace, spaceship, and credits backgrounds | No corresponding original screen background lookup in CivCraft's current renderer. |
| `Art/Diplomacy/counter.pcx`, `Art/Diplomacy/talk_offer.pcx` | Diplomacy panels | CivCraft does not load these official diplomacy sprites in its current client. |

### CivCraft's 92 campaign unit INI roots

The middle column comes from the checked-in Conquests unit catalog. Convert labels as `base → <INSTALL>/Art/Units/`, `ptw → <INSTALL>/civ3PTW/Art/Units/`, and `conquests → <INSTALL>/Conquests/Art/Units/`. Append `<art_folder>/<art_folder>.INI` for the catalog's selected default INI. For OpenCiv3, the comparable request is `Art/Units/<PediaIcons art name>/<PediaIcons art name>.INI`; its resolver tries Conquests, PTW, then base paths, so it can pick a higher-precedence same-named folder if multiple editions contain it. Its name can differ from CivCraft's `art_folder` and is derived from imported PediaIcons/rules data.

| Conquests identity | CivCraft selected official root | CivCraft folder name |
| --- | --- | --- |
| Settler | `base` | `Settler` |
| Worker | `base` | `Worker` |
| Scout | `base` | `Scout` |
| Explorer | `base` | `Explorer` |
| Marine | `base` | `Marine` |
| Modern Paratrooper | `base` | `Paratrooper` |
| Warrior | `base` | `Warrior` |
| Archer | `base` | `Archer` |
| Spearman | `base` | `Spearman` |
| Swordsman | `base` | `Swordsman` |
| Chariot | `base` | `Chariot` |
| Horseman | `base` | `Horseman` |
| Pikeman | `base` | `Pikeman` |
| Longbowman | `base` | `Longbowman` |
| Musketman | `base` | `Musketman` |
| Knight | `base` | `Knight` |
| Rifleman | `base` | `Rifleman` |
| Cavalry | `base` | `Cavalry` |
| Infantry | `base` | `Infantry` |
| Tank | `base` | `Tank` |
| Mech Infantry | `base` | `Mech Infantry` |
| Modern Armor | `base` | `Modern Armor` |
| Catapult | `base` | `Catapult` |
| Cannon | `base` | `Cannon` |
| Artillery | `base` | `Artillery` |
| Radar Artillery | `base` | `Radar Artillery` |
| Cruise Missile | `base` | `Cruise Missile` |
| Tactical Nuke | `base` | `Tactical Nuke` |
| ICBM | `base` | `ICBM` |
| Galley | `base` | `Galley` |
| Caravel | `base` | `Caravel` |
| Frigate | `base` | `Frigate` |
| Galleon | `base` | `Galleon` |
| Ironclad | `base` | `Ironclad` |
| Transport | `base` | `Transport` |
| Carrier | `base` | `Carrier` |
| Submarine | `base` | `Submarine` |
| Destroyer | `base` | `Destroyer` |
| Battleship | `base` | `Battleship` |
| AEGIS Cruiser | `base` | `Aegis Cruiser` |
| Nuclear Submarine | `base` | `Nuclear Submarine` |
| Fighter | `base` | `Fighter` |
| Bomber | `base` | `Bomber` |
| Helicopter | `base` | `Helicopter` |
| Jet Fighter | `base` | `Jet Fighter` |
| Stealth Fighter | `base` | `Stealth Fighter` |
| Stealth Bomber | `base` | `Stealth Bomber` |
| Leader | `base / base / base / base` | `Leader Ancient Times / Leader Middle Ages / Leader Industrial Ages / Leader Modern Times` |
| Army | `base / base / base / base` | `Army Ancient Times / Army Middle Ages / Army Industrial Ages / Army Modern Times` |
| Jaguar Warrior | `base` | `Jaguar Warrior` |
| Bowman | `base` | `Bowman` |
| Hoplite | `base` | `Hoplite` |
| Impi | `base` | `Impi` |
| Legionary | `base` | `Legionary` |
| Immortals | `base` | `Immortals` |
| War Chariot | `base` | `War Chariot` |
| Rider | `base` | `Rider` |
| Mounted Warrior | `base` | `Mounted Warrior` |
| Musketeer | `base` | `Musketeer` |
| Samurai | `base` | `Samurai` |
| War Elephant | `base` | `War Elephant` |
| Cossack | `base` | `Cossack` |
| Panzer | `base` | `Panzer` |
| Man-O-War | `base` | `Man-O-War` |
| F-15 | `base` | `F-15` |
| Privateer | `base` | `Privateer` |
| Keshik | `ptw` | `Keshik` |
| Conquistador | `ptw` | `Conquistador` |
| Berserk | `ptw` | `Berserk` |
| Sipahi | `ptw` | `Sipahi` |
| Gallic Swordsman | `ptw` | `Gallic Swordsman` |
| Ansar Warrior | `ptw` | `Ansar Warrior` |
| Numidian Mercenary | `ptw` | `Libyan Mercenary` |
| Hwach'a | `ptw` | `Hwacha` |
| Medieval Infantry | `ptw` | `Medieval Infantry` |
| Guerilla | `ptw` | `Guerilla` |
| Enkidu Warrior | `conquests` | `Enkidu Warrior` |
| Three-Man Chariot | `conquests` | `Three Man Chariot` |
| Carrack | `conquests` | `Carrack` |
| Swiss Mercenary | `conquests` | `Swiss Mercenary` |
| Trebuchet | `conquests` | `Trebuchet` |
| Chasqui Scout | `conquests` | `Chasquis Scout` |
| Javelin Thrower | `conquests` | `Javelin Thrower` |
| Dromon | `conquests` | `Dromon` |
| Cruiser | `conquests` | `Cruiser` |
| Crusader | `conquests` | `Crusader` |
| Ancient Cavalry | `conquests` | `Ancient Cavalry` |
| Curragh | `conquests` | `Curragh` |
| Paratrooper | `conquests` | `Paratrooper` |
| TOW Infantry | `conquests` | `TOW Infantry` |
| Flak | `conquests` | `Flak` |
| Mobile SAM | `conquests` | `Mobile SAM` |

For example, CivCraft's Berserk row resolves its selected INI below `civ3PTW/Art/Units/Berserk/Berserk.INI`; OpenCiv3 requests `Art/Units/Berserk/Berserk.INI`, which normally selects the Conquests candidate if present, otherwise the PTW path. The INI then selects each action FLC by filename, so exact animation files are a second path edge to reconcile after the INI path.

## Practical reconciliation checklist

1. For every path row above, record the actual resolved full path at runtime, not only the logical relative name. Include root, file spelling, extension, and whether the selected file came from Conquests, PTW, base, a scenario, or OpenCiv3/CivCraft packaged assets.
2. Compare image-sheet crop coordinates and sprite ID ordering. A common filename can still produce different sprites if crop cells, palette transparency, shadow processing, or terrain IDs differ.
3. For each unit catalog row, compare the selected INI path, then compare the INI's `[Animations]` entries action-by-action. Record missing/default-fallback actions, era-specific variants, and shared FLC references that walk to another unit folder.
4. Keep intentionally different rendering paths explicit: CivCraft-generated unit portraits, five city styles, derived color tint, and code-drawn UI are not source-path mismatches unless parity requires the official sheets.
5. Treat `source/` files in this checkout as the local reference installation only. Validate the table against a clean official Conquests/Complete installation before using it as a universal install manifest; scenario and Steam packaging can add or relocate assets.
