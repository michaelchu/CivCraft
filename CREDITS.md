# Original files used

The map renderer reads Civ III's `Art/Terrain/FogOfWar.pcx` from the sibling
source folder and uses it as a 9×9 visibility-transition alpha atlas. Unit
animations and the non-map unit portraits load from the original Civ III,
Play the World, and Conquests `Art/Units` folders.

| Project file | Original source path | Use |
| --- | --- | --- |
| Runtime unit art | `source/Art/Units`, `source/civ3PTW/Art/Units`, `source/Conquests/Art/Units` | Campaign unit animations and portraits |

No logo, splash screen, flag, nation emblem, music, or sound file is included. Selection, status markers, routes and combat effects use Civ III art or vector drawing.

## Civilization identities

The six fixed identities and leaders follow [Civilization III’s roster](https://civfanatics.com/civ3/infocenter/). City-name spellings and ordering follow the Civilization Wiki’s Civ III lists for [Romans](https://civilization.fandom.com/wiki/Roman_cities_(Civ3)), [Greeks](https://civilization.fandom.com/wiki/Greek_cities_(Civ3)), [Egyptians](https://civilization.fandom.com/wiki/Egyptian_cities_(Civ3)), [Babylonians](https://civilization.fandom.com/wiki/Babylonian_cities_(Civ3)), [Chinese](https://civilization.fandom.com/wiki/Chinese_cities_(Civ3)) and [Japanese](https://civilization.fandom.com/wiki/Japanese_cities_(Civ3)). The default build uses the artwork documented above. The optional Civ III
tileset loads original game PCX terrain from `Art/Terrain/` by default. The
selectable Snoopy terrain variants load from `Tilesets/<variant>/Art/Terrain/`.
