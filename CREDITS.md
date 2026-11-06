# Original files used

The following artwork comes from the local Freeciv source checkout. Original artist and source information is retained below.

| Project file | Original source path | Use |
| --- | --- | --- |
| assets/amplio2_terrain1.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/terrain1.png | Isometric land tile textures |
| assets/amplio2_terrain2.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/terrain2.png | Forest terrain overlays |
| assets/amplio2_hills.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/hills.png | Connected hill terrain sprites |
| assets/amplio2_mountains.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/mountains.png | Connected mountain terrain sprites |
| assets/amplio2_ocean.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/ocean.png | Ocean and coastline terrain sprites |
| assets/amplio2_water.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/water.png | Retained original four-direction river atlas |
| assets/hexemplio_rivers.png and .spec | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/hexemplio/rivers.png and rivers.spec | Six-direction river artwork, six outlets and original sprite definitions |
| assets/amplio2_select.png and .spec | Freeciv data/amplio2/select-alpha.png and select.spec | Four-frame unit selection ring; Jason Dorje Short, Eleazar / J. W. Bjerk, qwm |
| assets/amplio2_units.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/units.png | Unit sprites |
| assets/amplio2_cities.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/cities.png | City sprites |
| assets/amplio2_bases.png | /Users/michaelchu/Documents/projects/civjs/reference/freeciv/data/amplio2/bases.png | Fortifications and airfield sprites |

The Amplio2 and Hexemplio specification files in the same checkout identify their artists and license the sheets under GPL v2. No logo, splash screen, flag, nation emblem, music, or sound file is included. The Hexemplio river sheet credits Yautja (pond/river sources) and GriffonSpade. The renderer adapts its hex footprints to the current diamond tiles at asset load. Fog, borders, panels, controls, minimap, markers, and effects are recreated with shapes and colours in Rust.
