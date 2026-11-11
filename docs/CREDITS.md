# Original files used

The map renderer reads Civ III's `Art/Terrain/FogOfWar.pcx` from the sibling
source folder and uses it as a 9×9 visibility-transition alpha atlas. Unit
animations and the non-map unit portraits load from the original Civ III,
Play the World, and Conquests `Art/Units` folders.

| Project file | Original source path | Use |
| --- | --- | --- |
| Runtime unit art | `../../source/Art/Units`, `../../source/civ3PTW/Art/Units`, `../../source/Conquests/Art/Units` | Campaign unit animations and portraits |

No logo, splash screen, flag, nation emblem, music, or sound file is included. Selection, status markers, routes and combat effects use Civ III art or vector drawing.

## Civilization identities

The 31 playable identities, leaders, ordered city names, starting technologies, and color assignments are extracted from the Conquests BIQ `RACE` and `RACE_CityName` records. OpenCiv3's extracted ruleset is the checked-in comparison reference; the original BIQ is the authority. See `data/rulesets/civ3_conquests/README.md` for field mapping and rendering notes. The default build uses the artwork documented above. The optional Civ III
tileset loads original game PCX terrain from `Art/Terrain/` by default. The
selectable Snoopy terrain variants load from `Tilesets/<variant>/Art/Terrain/`.
