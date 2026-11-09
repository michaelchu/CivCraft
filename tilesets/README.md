# Terrain asset sources

The original Civ III terrain files are read directly from the sibling `source/`
folder. The default set uses `source/Conquests/Art/` first, then checks
`source/civ3PTW/Art/` and `source/Art/` for missing files.

PTW Snoopy terrain variants are read from their original scenario structure:

- `source/civ3PTW/Scenarios/Sn00pys Terrain/Art/`
- `source/civ3PTW/Scenarios/Sn00pys TerrainG/Art/`
- `source/civ3PTW/Scenarios/Sn00pys TerrainX/Art/`

Select one with `--tileset snoopy-standard`, `--tileset snoopy-greener`, or
`--tileset snoopy-extended`. The loader reads the original PCX files and handles
mixed-case filenames. No terrain sheets need to be copied into this directory.
