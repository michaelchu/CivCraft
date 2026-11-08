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
