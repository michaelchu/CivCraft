# Original files used

The following artwork comes from the local Freeciv source checkout. Original artist and source information is retained below.

| Project file | Original source path | Use |
| --- | --- | --- |
| assets/amplio2_fog.png and .spec | Freeciv data/amplio2/fog.png and fog.spec | Three-state corner fog transitions; Brendon Oram (Yautja) |
| assets/amplio2_terrain1.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/terrain1.png | Isometric land tile textures |
| assets/amplio2_terrain2.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/terrain2.png | Forest terrain overlays |
| assets/amplio2_hills.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/hills.png | Connected hill terrain sprites |
| assets/amplio2_mountains.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/mountains.png | Connected mountain terrain sprites |
| assets/amplio2_ocean.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/ocean.png | Ocean and coastline terrain sprites |
| assets/amplio2_water.png and .spec | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/water.png | Original lake shores and ice cliffs; legacy four-direction river atlas |
| assets/hexemplio_rivers.png and .spec | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/hexemplio/rivers.png and rivers.spec | Six-direction river artwork, six outlets and original sprite definitions |
| assets/amplio2_select.png and .spec | Freeciv data/amplio2/select-alpha.png and select.spec | Four-frame unit selection ring; Jason Dorje Short, Eleazar / J. W. Bjerk, qwm |
| assets/amplio2_units.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/units.png | Unit sprites |
| assets/amplio2_cities.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/cities.png | City sprites |
| assets/amplio2_tiles.png, amplio2_activities.png, amplio2_veterancy.png and .spec files | Freeciv data/amplio2/tiles, activities, veterancy | Health, activity, automation and veteran overlays; original artist credits retained in the companion specs |
| assets/amplio2_explosions.png and .spec | Freeciv data/amplio2/explosions | Battle casualty animation; xgalaga artwork by Joe Rumsey and Mike McGrath |
| assets/amplio2_cities.spec, civ2civ3_styles.ruleset, civ2civ3_city_art.ruleset | Freeciv data/amplio2/cities.spec and civ2civ3/styles.ruleset, effects.ruleset | Reference sprite tags, city styles and verbatim City_Image / Visible_Walls effect sections |
| assets/amplio2_terrain2.spec, amplio2_hills.spec, amplio2_mountains.spec | Freeciv data/amplio2 companion specifications | Connected terrain sprite tags and original artist credits |
| assets/amplio2_bases.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/bases.png | Fortifications and airfield sprites |

The Amplio2 and Hexemplio specification files in the same checkout identify their artists and license the sheets under GPL v2. No logo, splash screen, flag, nation emblem, music, or sound file is included. The Hexemplio river sheet credits Yautja (pond/river sources) and GriffonSpade. The renderer adapts its hex footprints to the current diamond tiles at asset load. Borders, panels, controls, minimap and faction banners use shapes and colours in Rust. Fog transitions, selection, status markers and battle explosions use the original reference artwork listed above.

## Civilization identities

The six fixed identities and leaders follow [Civilization III’s roster](https://civfanatics.com/civ3/infocenter/). City-name spellings and ordering follow the Civilization Wiki’s Civ III lists for [Romans](https://civilization.fandom.com/wiki/Roman_cities_(Civ3)), [Greeks](https://civilization.fandom.com/wiki/Greek_cities_(Civ3)), [Egyptians](https://civilization.fandom.com/wiki/Egyptian_cities_(Civ3)), [Babylonians](https://civilization.fandom.com/wiki/Babylonian_cities_(Civ3)), [Chinese](https://civilization.fandom.com/wiki/Chinese_cities_(Civ3)) and [Japanese](https://civilization.fandom.com/wiki/Japanese_cities_(Civ3)). The default build uses the artwork documented above. The optional Civ III
tileset loads its PNGs from `tilesets/civ3/` at runtime.
