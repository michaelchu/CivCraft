# Local map tilesets

Each theme has its own directory. The default Civ III theme is in `civ3/`,
with its terrain sheets in `civ3/Terrain/` and its resource sheets at the
root of the theme directory.

Launch the default Civ III theme with `--tileset civ3`. To select a different
Civ III theme, use `--tileset civ3 --tileset-theme tilesets/my-theme`.
Copy the example manifest from `civ3/tileset.conf` when overriding sheet paths.
River art can be overridden with `river-delta` and `river-mountain`; these
roles select the original 4×4 Civ III river sheets. See
[Map tilesets](../README.md#map-tilesets) for the supported sheet roles.
Road and railroad art can be overridden with the `roads` and `railroads`
roles. The bundled route sheets come from the checked-out OpenCiv3
`C7/Assets/Art/Terrain/` folder because the Sn00py terrain set does not include
them. The original 16×16 atlas layout is kept, and PCX or PNG mod sheets can
replace either role through the manifest.
The `TerrainBuildings.png`, `pollution.png`, and `craters.png` fallback sheets
also come from OpenCiv3's `C7/Assets/Art/Terrain/` folder; Sn00py does not
include those sheets. OpenCiv3's `terrain_improvements.lua` maps the 128×64
fortress cell at column 0, row 0 and mine at column 2, row 1. The pollution
and crater sheets use 128×64 cells in 5×5 grids in its `TileOverlayLayer.cs`.
These are implementation-reference mappings, not proof of Civ III internals.
Themes can override them with `terrain-buildings`, `pollution`, and `craters`
roles using PCX or PNG files. Contamination currently chooses stable single-tile
variants; connected pollution/crater stitching remains to be implemented.
Indexed PCX images use palette entries 254 and 255 as transparent by default,
matching the OpenCiv3 loader. A mod can override those indices for an image
role with a comma-separated `transparent-<role> = 254,255` manifest entry.
