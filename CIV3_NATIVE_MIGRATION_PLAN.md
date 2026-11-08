# Civ III Native Migration Plan

Status: active migration plan, written 2026-11-08. This plan defines the
intended end state; a completed phase requires the evidence listed below.

## Authority and evidence

- The Civ III manual in `civ3_manual.pdf` is the gameplay authority. Its
  introduction describes the map as an isometric grid of diamond tiles with
  movement in eight compass directions (printed page 8). When the manual does
  not specify an implementation detail, record that detail as an inference.
- `../../OpenCiv3` is an implementation reference, not proof of original game
  internals. Its `C7/MapView.cs` uses 64×32 map cells for 128×64 tile art,
  staggers occupied tile coordinates by row, and contains screen-to-tile
  picking. `C7Engine/C7GameData/Tile_Math.cs` defines eight directions on that
  staggered coordinate grid. Treat these choices as a reference model and
  compare the visible result with Civ III documentation and artwork.
- `../../Sn00pys Terrain Greener (C3C)/Art` is the default terrain-mod source.
  Keep its PCX files as the runtime inputs; do not make generated PNGs a
  prerequisite.
- Freeciv code, data and existing CivCraft behavior are migration context only.
  They do not define the target rules or rendering behavior.

## Current verified baseline

- Civ III terrain art can be loaded through the existing shared map draw path.
- `src/client/tileset.rs` now decodes 8-bit, one-plane RLE PCX, treats magenta
  as transparent, checks PCX extensions when PNG defaults are absent, and
  accepts city-style manifest roles. Default tileset selection prefers the
  neighboring Sn00py `Art` directory when present and falls back to the bundled
  Civ III sheets.
- Civ III artwork and the Civ III geometry backend are now the CLI defaults.
  `--renderer amplio2` remains available for comparison. The Civ III backend
  uses staggered-grid projection and picking but still shares the old painter.
  Simulation tile indexing, topology, and most terrain choices remain legacy.
  This is not yet a native Civ III renderer or world model.
- Five focused PCX/tileset tests passed, the debug build succeeded, and the
  `civ3-city-sprites` UI review passed with the default tileset at seed 42.
- `src/client/civ3_geometry.rs` holds a first-pass Civ III grid contract:
  parity-valid staggered coordinates, flat indexing, eight reciprocal
  neighbors, horizontal wrapping, pixel centers, and diamond picking.
  Seven focused geometry tests pass.
- `--renderer civ3` now selects a Civ III geometry backend for projection,
  tile picking, visible-tile placement, and edge-water placement. It still
  shares the existing map painter and simulation. The backend is opt-in and
  does not make the complete Civ III renderer phase complete.
- The full UI review under `--renderer civ3` passed at seed 42: 47 scenes,
  146/146 checks, and 68 captures. The report is
  `/tmp/civ3-renderer-gallery-final/report.json`. Terrain blending, resources,
  both poles, city sprites, and map layers were visually inspected. The painter
  still exposes visible transition/layer mismatches that need to be addressed
  in the dedicated painter phase.
- After making Civ III the default, the full 47-scene gallery passed at seed
  42 with `renderer: civ3`; report and captures are in
  `/tmp/civ3-default-review/`. The terrain-blending screenshot reproduces
  hard-edged, saturated water-depth wedges and an overly uniform grass base
  beneath varied overlays. This confirms the shared painter/model mismatch is
  visible even when automated gallery assertions pass.
- Two stale visual fixtures assumed vertical wrapping despite the bounded-pole
  default. The remembered-map scout now moves to another valid tile, and the
  water-depth fixture uses campaign-aware offsets and stays inside the map.
- The full Rust suite passed 610/610 after making the Civ III backend the
  default. The release build and complete default-backend UI gallery passed;
  `git diff --check` is clean.

## Phases and completion criteria

### 1. Civ III geometry contract

Create a renderer-independent Civ III geometry module for staggered tile
coordinates, map dimensions, wrapping, tile centers, visible-row iteration,
and screen-to-tile picking. Keep it separate from the existing hex topology
until callers can migrate deliberately.

Complete when:

- Coordinate validity and neighbor offsets agree across all eight directions,
  including map seams and bounded top/bottom edges.
- Forward projection and picking agree for every tile center and for points on
  each tile's diamond edges/corners at integer and fractional zoom.
- Tests cover even/odd rows, wrapped and bounded maps, and edge tiles without
  relying on Amplio2 projection helpers.
- The module's documented rules are tied to manual page 8 where documented;
  any storage/indexing choices inferred from OpenCiv3 are explicitly labeled.

### 2. Dedicated Civ III map renderer

Add a `Civ3Renderer` that owns map projection, visible-tile collection, painter
order, and tile picking. Draw the Civ III 128×64 diamonds on the staggered
64×32 cell grid. Keep HUD code shared only where it is independent of map
geometry. Route clicks, hover, camera movement, minimap navigation, city-map
input, and map overlays through the selected backend.

Complete when:

- Both drawing and input use Civ III geometry and no call path silently falls
  back to Amplio2 projection or tile indexing.
- Screenshot review covers land/water transitions, Sn00py PCX art, resources,
  cities, units, polar edges, wrapped horizontal seams, and the city work map.
- The Civ III renderer is now the default. Complete the remaining paths before
  removing the Amplio2 comparison backend.

### 3. Mod-ready PCX asset pipeline

Finish the theme manifest and PCX semantics in `src/client/tileset.rs`. Every
terrain sheet, water-depth sheet, overlay, resource sheet and shadow sheet,
polar sheet, and city style must be replaceable through a manifest. Preserve
transparent keys and shadow data according to each sheet's role; do not flatten
resource shadows into ordinary terrain or discard palette information needed
by keyed artwork. Keep PNG support as an optional theme format.

Complete when:

- The complete Sn00py set loads directly from its PCX files, including city
  styles, resource shadows, overlays, terrain, water and polar art.
- A second manifest can replace each role independently without renderer code
  changes; missing required files and malformed PCX produce path-specific
  errors.
- Decoder checks include malformed/truncated data, row padding, RLE boundaries,
  color-key pixels, palette conversion, and representative real Sn00py assets.
- Screenshots show no magenta key fringes, opaque atlas grids, or missing
  resource shadows.

### 4. Civ III terrain and layer behavior

Make base terrain identities, coast/sea/ocean bands, terrain transitions,
vegetation, hills, mountains, rivers, improvements, resources, and polar art
data-driven. Use original art sheets and documented Civ III behavior. Maintain
separate evidence notes for art-sheet interpretation and gameplay rules.

Complete when:

- Every terrain and overlay state supported by the simulation has an explicit
  art mapping or a documented unsupported reason.
- Water depth and coast adjacency match the intended Civ III model; edge and
  seam screenshots show continuous, correctly layered artwork.
- Resource and overlay placement keeps the right source artwork and shadow
  channel through the map, minimap, fog, and city work map.

### 5. World model and gameplay topology

Migrate storage/indexing and every topology-dependent caller to the Civ III
staggered square grid and eight directions. Audit movement, pathfinding,
retreats, city radii, worked tiles, visibility, borders, rivers, trade/fuel
routes, unit automation, combat adjacency, and map interaction. Remove the old
six-direction `ISOHEX_DIRS` assumptions as each subsystem is migrated.

Complete when:

- Neighbor relations are reciprocal and consistent at edges and wrap seams.
- Movement costs, legal moves, routes, visibility, city radius and river/border
  connections use the same eight-direction geometry.
- Scenario checks exercise each migrated subsystem on ordinary tiles, all map
  edges, and the horizontal seam.
- Save compatibility is not used to constrain the new representation; replace
  the save format or migrations if needed.

### 6. Civ III map generation

Replace the Freeciv-derived generator as the source of generated Civ III
campaigns. Implement Civ III map size, landmass, water coverage, climate,
temperature, and age settings, then terrain assignment, elevation, coast
classification, rivers, resources, and starting locations. Distinguish manual
facts from reverse-engineered or approximated details and preserve seed
determinism.

Complete when:

- Each setting has a documented Civ III target and an implementation mapping.
- Deterministic checks cover representative setting combinations and seeds,
  including polar and coastal maps; golden data encodes Civ III expectations,
  not raw Freeciv output.
- Screenshots are inspected for landmass shape, climate bands, terrain mixes,
  rivers, resource distribution, start locations, poles, and wrap seams.

### 7. Amplio2 retirement and final audit

Remove the Amplio2 renderer, projection, art loading, compatibility branches,
and obsolete checks after all runtime paths and fixtures use the Civ III
implementation. Remove stale Freeciv behavior only after the corresponding
Civ III replacement has evidence.

Complete when:

- A source and configuration audit finds no Amplio2 runtime dependency or
  Freeciv-derived target behavior left in scope.
- The default executable starts with Civ III renderer and Sn00py theme without
  opt-in flags or PNG conversion.
- Automated tests, requested gameplay scenarios, and visual galleries pass;
  screenshots have been inspected at normal and compact sizes and at map
  boundaries.
- The final report lists completed systems, validation evidence, remaining
  fidelity gaps, and all behavior still based on inference.

## Migration order

Proceed in this order: geometry contract, dedicated renderer, complete PCX/theme
pipeline, terrain behavior, gameplay topology, map generation, then Amplio2
retirement. Some asset and terrain work can proceed alongside geometry, but do
not make the old renderer's projection the permanent Civ III abstraction.
Keep changes small and reviewable. Do not push unless explicitly requested.
