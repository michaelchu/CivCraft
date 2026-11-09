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
- OpenCiv3's `C7/MapView.cs` `RiverLayer` selects a 4×4 river sprite from flags
  on the four tiles around each junction. Its `C7Engine/MapGenerator.cs`
  `setRiverFlags` comments explicitly call that code a rough approximation
  with missing cardinal flags. Reuse the junction layout as an implementation
  reference, not as a claim about the original generator.
- OpenCiv3's `C7/Map/TileOverlayLayer.cs` chooses one of 16 irrigation cells
  from the four diagonal neighbors. The file explicitly says its direction
  flags may be wrong and cites `civ3_cross_platform_editor` for those values.
  CivCraft follows that mapping as an implementation approximation, not
  verified original-engine behavior.
- The same OpenCiv3 file maps all eight route directions to bit flags and crops
  128×64 cells from a 16×16 road/railroad atlas. CivCraft uses those flags and
  atlas dimensions as an implementation reference, not proof of original game
  internals. The Sn00py terrain folder has no road sheets, so the bundled route
  PNGs are copied from OpenCiv3's `C7/Assets/Art/Terrain/`; manifests can replace
  them with another mod's PCX or PNG sheets.
- `../../Sn00pys Terrain Greener (C3C)/Art` is the default terrain-mod source.
  Keep its PCX files as the runtime inputs; do not make generated PNGs a
  prerequisite.
- Freeciv code, data and existing CivCraft behavior are migration context only.
  They do not define the target rules or rendering behavior.

## Current verified baseline

- Civ III terrain art can be loaded through the existing shared map draw path.
- OpenCiv3 marks `C7/Assets/Art/Units/units_32.png` as placeholder artwork;
  its map-unit path loads directional `.flc` animations. CivCraft now has a
  Rust decoder for the FLC chunk types used by the inspected OpenCiv3 reader
  (palette, byte-run, and delta frames), and Civ III theme manifests accept
  `unit-<kind>` entries that point directly to FLC or Civ III unit `.ini` files,
  plus optional `-run` and `-attack` action roles. The loader reads the `.ini`
  `DEFAULT`, `RUN`, and `ATTACK1` references and resolves their FLCs relative
  to that file. The decoder packs FLC direction and frame rows into base/tint
  atlases.
  A seed-42 world capture with a configured Carthaginian Settler FLC confirms a
  rendered palette/team-color frame:
  `/tmp/civ3-unit-flc-review-settlers/world.png` and report
  `/tmp/civ3-unit-flc-review-settlers/report.json`. This proves the optional
  loader path only; the default theme does not map unit art, action animation
  state mapping is partial, and most units still use the shared atlas. The six
  legacy movement directions now select the corresponding FLC facing row, but
  the gameplay topology still needs all eight directions. OpenCiv3 unit artwork
  is treated as reference/mod art rather than verified original Civ III art.
  `C7/Animations/AnimationManager.cs` defines the FLC row order as southwest,
  south, southeast, east, northeast, north, northwest, west. CivCraft stores
  that row on the unit after movement or an attack; its current six-direction
  topology can therefore reach only six of those rows. The conversion is an
  implementation mapping from CivCraft's current coordinates, not a claim about
  Civ III's original movement internals. The seed-42 `unit-movement` UI review
  performs a real simulation move and shows the run animation resolved from an
  OpenCiv3 Warrior `.INI` with a single `unit-warrior` manifest entry. It passed
  5/5 checks and produced
  `/tmp/civ3-unit-ini-animation-review/unit-movement.png` and
  `/tmp/civ3-unit-ini-animation-review/unit-movement-next.png`.
- `src/client/tileset.rs` decodes 8-bit, one-plane RLE PCX and applies palette
  indices 254 and 255 as transparent by default, following OpenCiv3's loader
  configuration. Theme manifests can override transparent indices per sheet
  role; magenta RGB keys remain supported. The decoder checks PCX extensions
  when PNG defaults are absent and accepts city-style manifest roles. Default
  tileset selection prefers the neighboring Sn00py `Art` directory when
  present and falls back to the bundled Civ III sheets.
- Civ III artwork and the Civ III geometry backend are now the CLI defaults.
  `--renderer amplio2` remains available for comparison. Civ III world-pass
  orchestration now lives in the `Civ3WorldPainter` in
  `src/client/world/civ3.rs`, and `Civ3Renderer` invokes it directly. The
  painter takes game state and assets explicitly but still delegates tile
  drawing to shared `Game` map-layer helpers; Amplio2 currently delegates to
  the same painter, so it is not an independent legacy renderer. Simulation
  storage/topology and most terrain choices remain incomplete. This is not yet
  a fully native Civ III renderer or world model.
- `Tile` now retains a separate `base_terrain` for Civ III transition art;
  generated forest, jungle, hill, mountain, marsh, and volcano tiles no longer
  have to use their feature name as the base sheet. The current generator
  infers these bases from broad temperature bands because it does not yet
  generate Civ III base terrain and overlays independently. This mapping is
  explicitly provisional and must be replaced by the native generator.
- Tiles retain reciprocal eight-direction `river_edges`. River source checks,
  course tracing, drainage scoring, merging, and outlets now use Civ III
  staggered neighbors through the storage adapter; generated edges are written
  directly from each course step. Civ III rendering reads the four-tile
  junction flags and loads Sn00py's `deltaRivers.pcx` and `mtnRivers.pcx`
  through the `river-delta` and `river-mountain` manifest roles. The drainage
  scoring remains a provisional heuristic over the current elevation field;
  it is not evidence of the original Civ III generator.
- Native irrigation rendering now selects the 4×4 PCX sheet cell from visible
  diagonal irrigated neighbors on the Civ III staggered grid, and selects the
  terrain-specific sheet from `base_terrain`. The direction-to-bit mapping is
  the OpenCiv3 approximation above. Farmland still uses the old fallback atlas
  because no dedicated farmland role or source cell has been identified in
  the supplied Sn00py art set.
- Native road and railroad rendering now uses the 16×16 Civ III route atlases
  and eight staggered neighbors. Railroad tiles draw road-only connections
  beneath railroad connections, following OpenCiv3's `TileOverlayLayer`; this
  remains an implementation reference pending direct original-game evidence.
  Theme manifests can replace either route sheet independently.
- Mines, forts/fortresses, pollution, and fallout now use the Civ III
  improvement-art path in `Civ3WorldPainter`. OpenCiv3's
  `terrain_improvements.lua` identifies the fortress (column 0, row 0) and mine
  (column 2, row 1) cells in `TerrainBuildings`; its PNG export is bundled
  because the supplied Sn00py set omits that sheet. OpenCiv3's pollution and
  crater atlases are also bundled as fallbacks and are independently
  replaceable through theme manifest roles. `TileOverlayLayer.cs` maps these
  to 128×64 cells in 5×5 grids. Pollution and crater variants currently use a
  4-bit diagonal-neighbor mask for connected cells and stable coordinate-based
  selection for isolated cells. The mask order and atlas layout follow
  OpenCiv3's implementation and remain an inference about original Civ III
  behavior; isolated-cell variation does not reproduce OpenCiv3's global RNG.
  The bundled `TerrainBuildings.png` export is only 3 columns wide, so its
  Lua-referenced barricade cell (column 3) is unavailable. Barbarian camps use
  column 2, row 0 as mapped in OpenCiv3's `terrain.lua`, and the game renders
  that as the hut sprite. Barricades therefore remain unsupported by this
  sheet. No source mapping was found for oil wells/platforms, airstrips, or
  airbases. They now have independent optional `oil-well`, `oil-platform`,
  `airstrip`, and `airbase` theme roles, each accepting a single transparent
  128×64 PCX or PNG sprite. Existing legacy art remains the fallback when a
  theme does not supply those files; retiring that fallback still remains.
- The focused `map-layers` UI review now includes mine, fortress, pollution,
  fallout, and a barbarian camp and passes all three scene checks at seed 42.
  The camp uses the OpenCiv3 `terrain.lua` cell mapping. Screenshots were
  inspected: `/tmp/civ3-camp-improvement-review/map-layers.png` and
  `/tmp/civ3-camp-improvement-review/map-layers-next.png`; report:
  `/tmp/civ3-camp-improvement-review/report.json`. The full Rust suite passes
  628/628 and `cargo build --offline --release` succeeds after the camp change.
- Terrain-generation adjacency now also uses Civ III's eight-direction
  neighbors: connected land/water regions, biome and water-distance queries,
  resource spacing, and start-location distances use the same staggered graph
  as river generation. Seeded CivCraft vectors were refreshed and focused
  topology/water/golden checks pass. This changes neighborhood behavior only;
  the elevation field, Freeciv-derived map RNG, and fixed campaign dimensions
  still need replacement.
- New-world generation now accepts Civ III's 60%, 70%, and 80% ocean-coverage
  choices; 70% is the default, and `--ocean-coverage` exposes them at launch.
  The setting changes the ranked elevation threshold and scales river targets
  with the resulting land percentage. Deterministic checks verify water
  coverage increases across the three choices. OpenCiv3's `WorldSetup.cs` and
  `GameSetup.cs` also default to 70%; this is an implementation-reference
  alignment. UI captures of seed 42 at 60%, 70%, and 80% were inspected at
  `/tmp/civ3-ocean-60/world.png`, `/tmp/civ3-ocean-70/world.png`, and
  `/tmp/civ3-ocean-80/world.png`; each report passed its scene check. The
  release build and full Rust suite (649/649) pass. Map dimensions remain
  fixed, and landform, climate, temperature, and age settings are not
  implemented.
- Water shelf correction now follows OpenCiv3's `FixContinentalShelf` passes:
  sea/ocean cells touching land become coast, then ocean cells touching coast
  become sea. This preserves the generated initial depth classes farther from
  shore, replacing the prior fixed distance bands. Tests verify the shoreline
  invariant in both wrap modes, and CivCraft generation vectors were refreshed
  separately from raw Freeciv references. This is an OpenCiv3 implementation
  reference; the manual does not document Civ III's internal shelf algorithm.
- River sheets previously displayed their index-254 bright-green background
  because the decoder only recognized magenta RGB. A focused capture after
  applying the PCX palette transparency indices shows the background removed;
  report and image: `/tmp/civ3-river-native-pcx-review/report.json` and
  `/tmp/civ3-river-native-pcx-review/rivers-generated.png`.
- The PCX checks cover palette-index transparency, magenta keys, row padding,
  resource shadows, and river-role overrides. The default PCX indices are
  based on OpenCiv3's loader configuration, not confirmed original-engine
  internals.
- `src/client/civ3_geometry.rs` holds a first-pass Civ III grid contract:
  parity-valid staggered coordinates, flat indexing, eight reciprocal
  neighbors, horizontal wrapping, pixel centers, and diamond picking.
  Seven focused geometry tests pass.
- `MapWrap::civ3_neighbors` exposes all eight reciprocal staggered-grid
  neighbors. Terrain alteration
  water thresholds, irrigation sources, and stranded-unit relocation use the
  Civ III neighbor API. Water thresholds use the available adjacent-cell count
  at bounded map edges. A full shared-iterator switch was tried and reverted:
  merchant ferry routing still expands legacy offsets, so mixing the two
  coordinate systems broke that route. Movement/pathfinding and other callers
  were migrated together before the legacy iterator could be retired.
- The wider simulation adjacency audit has now migrated its remaining direct
  six-neighbor scans in continent labeling, advisors, city defense,
  paratrooper targeting, resource connectivity, disasters, counterintelligence,
  guards, movement ports, offense targeting, settlement sites, merchant
  landings, trade continents, and uprising placement. The last simulation use
  of `MapWrap::neighbors` was removed. The merchant ferry scenario exposed a
  separate leftover: delivery eligibility and land approaches still used the
  old map-distance metric, so a merchant landed on a valid Civ III adjacent
  tile but could not deliver. Those checks and lost-home destination ranking
  now use `civ3_distance`; the full 645-test suite and offline release build
  pass. Other uses of the legacy `map_distance` API remain to be audited for
  topology-sensitive gameplay behavior.
- City and unit interaction ranges in population joining, disbanding, embassy
  establishment, city investigation and covert actions, bribery, diplomatic
  contact, and AI counterintelligence now use `civ3_distance`. This removes
  the old vertical-row adjustment from one-tile action ranges; the gameplay
  suite passes with these checks on the Civ III grid. Wider strategic ranges
  and remaining AI distance heuristics are still unaudited.
- Tactical adjacency and proximity now use `civ3_distance` for invasion
  support, amphibious target selection, attack helpers, guard following and
  rendezvous, ferry escort state, settlement safety, aircraft follow-up, and
  zones of control. Worker threat safety also measures its two-tile radius on
  the Civ III graph. The prior ISOHEX-radius test now places units by Civ III
  steps; focused coverage and the full 645-test suite pass. Some wider strategic
  guard and target-selection heuristics still use the legacy distance metric.
- Nuclear blast coverage now uses the target square and eight Civ III
  neighbors, and each exposed unit gets the manual's 50% survival roll. The
  manual documents the affected squares and unit survival chance on printed
  page 84. The prototype SDI interception radius of two Civ III steps remains
  an inference; the manual identifies SDI Defense as the defense against most
  nuclear attacks but does not specify its range or probability.
- The shared `Simulation::map_distance` now delegates to `MapWrap::civ3_distance`,
  so remaining strategic range, ranking, and placement callers use Civ III graph
  steps instead of the old ISOHEX vector metric. Observer test fixtures now
  place targets by those graph distances; all 645 tests pass. This does not
  finish topology alignment: visibility revelation, exploration scoring,
  village discovery masks, and several UI/test helpers still construct ranges
  with explicit ISOHEX offsets. The city minimum-spacing value also needs a
  Civ III source before its current numeric threshold can be treated as final.
- Live map revelation, bounded exploration scoring, and village map rewards now
  enumerate a bounded BFS radius on the Civ III neighbor graph. Barbarian-village
  escape and transport neighbors, and espionage route projection, also use Civ
  III direction steps. A radius helper check compares its results against full
  distance fields at map corners and wrapping edges; the expanded 646-test suite
  passes. Remaining explicit ISOHEX use is mostly comparison/UI fixture code,
  while worker road-value heuristics and the compatibility move command remain
  candidates for review.
- The live post-bribery move now selects a Civ III direction, and worker road
  improvement scoring checks the eight Civ III neighbors instead of twelve
  legacy hex/map offsets. The obsolete wrapped map-vector adapter was removed
  after its last gameplay caller migrated. The regular keyboard path already
  emits Civ III directions; the old `Command::Move(dx, dy)` remains only as a
  compatibility path for playtest and legacy unit scenarios and still needs
  retirement or fixture migration.
- Units displaced by a traded city now search a radius of two Civ III graph
  steps for a legal destination instead of scanning legacy map-coordinate
  offsets. All 647 Rust tests pass after this migration.
- Human directional movement now has a Civ III direction-index command for all
  eight staggered-grid neighbors. Arrow keys and the numeric keypad select the
  corresponding Civ III steps, and FLC facing rows follow OpenCiv3's direction
  order. The human `GoTo` planner, path follower, and route preview now use the
  same eight-direction graph. AI normal-path expansion and reconstruction now
  use that graph too; the normal-path executor, ground-invasion executor,
  barbarian-leader routes, and worker routes follow Civ III direction steps.
  City embark scope and worker naval-safety adjacency use the same neighbors.
  Route tests and fixtures calculate Civ III tile positions and eight-way
  costs. PF_FUEL expansion, segment reconstruction, return-to-base distance,
  and the shared AI path executor now use the Civ III graph. Refuel wait
  markers remain explicit and stop execution at the planned refuel boundary.
  Automated exploration, its movement follower, unknown-tile adjacency
  scoring, and generic unit wandering now use Civ III neighbors. The
  uncertainty weight is scaled over eight neighbors; that behavior is an
  implementation inference, not a manual-documented Civ III AI rule. The
  shared `search_route` and route-cost APIs now search the Civ III graph;
  bribery, settlement, guard-rampage, and espionage path execution uses the
  Civ III follower. Settlement-ferry sea search, settler walking, landing,
  inland route reconstruction, and merchant passenger pickup and inland
  delivery landing now use Civ III directions too. Assault-transport target
  selection, shore approach, landing, and amphibious attack now use the same
  Civ III directions, including shared ground-invasion target selection.
  Coastal-port detection, trireme access checks, and adjacent embarkation also
  use all eight Civ III neighbors; this fixed the settlement ferry's failure
  to board at diagonal coastal approaches.
  Other legacy topology callers remain outside these route planners and need
  their own coordinated migrations.
  Tests cover direct movement, route preview, wrapped seams, FLC facing rows,
  and migrated AI routes; `cargo test --offline` passes 645/645 and the
  offline release build succeeds. Border and route playtest fixtures now use
  isolated city state, Civ III rank distance, and routes built from Civ III
  neighbors. The seed-42 desktop scenario passes 271/278 checks; the seven
  remaining failures match the earlier migration baseline. Report:
  `/tmp/civcraft-playtest-d3YwZD/report.json`. The Civ III distance formula
  used for fuel return range matches graph traversal for sampled origins across
  both wrap modes; its use for original Civ III fuel-range behavior remains an
  inference.
  The focused `goto-preview` Civ III UI review passed
  4/4 checks, and its screenshot was inspected at
  `/tmp/civ3-goto-eight-direction-review/goto-preview.png`; report:
  `/tmp/civ3-goto-eight-direction-review/report.json`.
- City-local building requirements (coast, river, lake, water) now traverse
  the explicit Civ III neighbor graph. Road connectivity for unit resources
  still follows the legacy iterator because route expansion has not migrated.
  The city requirement fixture includes a diagonal neighbor, and the AI
  city-defense fixture clears adjacent rivers so it tests a landlocked city on
  the expanded neighborhood. Full `cargo test --offline` passes 638/638.
- Climate moisture preference now also checks all eight Civ III neighbors,
  with a test for diagonal coastal water. This changes topology input to the
  existing CivCraft climate heuristic; the heuristic itself is not claimed to
  reproduce original Civ III climate-change mechanics.
- City work areas now select 21 distinct valid staggered-grid tiles with a
  rank-two Civ III shape; the center plus 20 surrounding tiles match the
  manual's chapter 6, printed page 55, which calls it a "fat X" and notes that
  a new city initially works only the nearest eight. The rank formula follows
  OpenCiv3 `Tile_Math.cs` (`RankDistanceTo`) and is an implementation
  reference, not verified original-engine source. The manual documents the
  count and shape, but not the formula. Both map wrap modes and the city-map
  panel use the same rank-two selection. Border and work eligibility now map
  the manual-described initial nearest-eight workable area to a rank-one
  claimed area using shortest paths over the Civ III eight-neighbor graph;
  interpreting that as initial ownership is inferred. City work eligibility
  requires the tile to belong to that city owner. Larger border expansion still uses city
  size as an explicit approximation because the current model does not yet
  implement Civ III culture thresholds. The manual documents the initial
  nearest-eight workable area and larger city radius; shortest-path rank and
  the size proxy are implementation choices. Focused
  regression coverage verifies the eight claims and rejects working an
  unowned rank-two tile in both wrap modes. Full `cargo test --offline` passes
  640/640. The focused `campaign-worked-tiles` UI review passed all 12 checks
  at seed 42 and its Civ III city radius screenshot was inspected:
  `/tmp/civ3-border-review/campaign-worked-tiles.png` (report:
  `/tmp/civ3-border-review/report.json`).
- The Civ III geometry backend controls projection, tile picking,
  visible-tile placement, and edge-water placement. `Civ3WorldPainter` in
  `src/client/world/civ3.rs` owns ordered world-pass orchestration and receives
  game state and assets explicitly. It owns base-art selection and the native
  Blend/Terrain layer decisions for both the world map and city work map; shared
  `Game` drawing remains as the fallback when no Civ III tileset is loaded or
  an overlay is missing. Other feature and overlay passes still call shared
  map-drawing helpers. The full Civ III renderer phase is incomplete.
- The full UI review under `--renderer civ3` passed at seed 42: 47 scenes,
  146/146 checks, and 68 captures. The report is
  `/tmp/civ3-renderer-gallery-final/report.json`. Terrain blending, resources,
  both poles, city sprites, and map layers were visually inspected. The painter
  still exposes visible transition/layer mismatches that need to be addressed
  in the dedicated painter phase.
- The focused `terrain-blending` scene passed with the new base-ground field;
  capture: `/tmp/civ3-base-ground-review-fixed/terrain-blending.png`. The
  fixture now assigns its expected base ground as well as feature terrain.
  Blue mottling in the water transition is present in the original `wCSO.pcx`
  sheet, so it is source artwork rather than an artifact to smooth away. Base
  terrain for feature tiles remains an inference until map generation stores
  it directly.
- The focused generated-river capture is
  `/tmp/civ3-generated-river-review/rivers-generated.png`. The full default
  Civ III gallery then passed all 47 scenes, 146 checks and 68 captures at seed
  42; report: `/tmp/civ3-river-gallery/report.json`. The focused review using
  the Sn00py `Art` directory passed at
  `/tmp/civ3-river-native-pcx-review/report.json`. The river-sheet capture was
  visually inspected after fixing its palette transparency. Rivers use Civ III
  source art in both fixture and generated-map scenes. Their placement remains
  approximate because both the elevation field and drainage score need Civ
  III-native evidence and implementation.
- Two stale visual fixtures assumed vertical wrapping despite the bounded-pole
  default. The remembered-map scout now moves to another valid tile, and the
  water-depth fixture uses campaign-aware offsets and stays inside the map.
- CivCraft's seeded map vectors are kept separate from raw Freeciv vectors and
  now use `generate_with_wrap` for both wrap modes, including the cylinder's
  polar-row postprocessing. River topology changes intentionally update these
  CivCraft vectors; they do not alter the raw Freeciv parity fixture.
- River topology and the campaign-aware settlement-neighbor fix passed the
  full Rust suite (619/619); the offline release build and focused rendered river
  review also pass. The generated river scene is at
  `/tmp/civ3-river-native-pcx-review/rivers-generated.png`.
- After routing terrain generation through Civ III's eight-way graph, the
  offline Rust suite passes 620/620 and the release build succeeds. The full
  default Civ III gallery passes 47 scenes, 146 checks, and 68 captures at
  seed 42; report: `/tmp/civ3-eight-way-review/report.json`. I inspected the
  new generated river/pole capture at
  `/tmp/civ3-eight-way-review/rivers-generated.png`.
- After moving ordered pass ownership into `Civ3WorldPainter`, `cargo check
  --offline`, `cargo test --offline` (620/620), and `cargo build --offline
  --release` pass. The default Civ III UI review passes 47 scenes, 146 checks,
  and 68 captures at seed 42; report:
  `/tmp/civ3-world-painter-final-review/report.json`. The world, city sprites,
  water-depth, and both pole captures were inspected. This structural change
  does not complete renderer independence.
- After moving base-art selection into `Civ3WorldPainter` for the world and
  city work maps, `cargo check --offline` and `cargo test --offline` (620/620)
  pass. The release build and seed-42 Civ III UI review pass; report:
  `/tmp/civ3-base-painter-review/report.json` (47 scenes, 146 checks, 68
  captures). The city work-map and terrain-blending captures were inspected.
  The no-tileset compatibility fallback remains in `Game` during migration.
- After moving native Blend/Terrain layer dispatch into `Civ3WorldPainter`,
  `cargo check --offline`, `cargo test --offline` (620/620), and the release
  build pass. The seed-42 gallery passes 47 scenes, 146 checks, and 68 captures;
  report: `/tmp/civ3-layer-painter-review/report.json`. World, terrain-blending,
  water-depth, and polar captures match the prior review fingerprints. City
  work-map checks pass; the capture varies between runs as its scripted state
  advances.
- After adding connected Civ III irrigation-sheet selection, `cargo check
  --offline`, the full Rust suite (622/622), the release build, and the default
  UI review pass. The gallery report is
  `/tmp/civ3-irrigation-final-review/report.json` (47 scenes, 146/146 checks,
  68 captures); its `map-layers.png` scene was inspected. Focused tests cover
  the diagonal flag mask and the 4×4 cell crop.
- After routing roads and railroads through the Civ III 16×16 atlases,
  `cargo check --offline`, the full Rust suite (625/625), the release build, and
  the default UI review pass. Report:
  `/tmp/civ3-route-atlas-review/report.json` (47 scenes, 146/146 checks, 68
  captures). The `map-layers.png` world scene and
  `map-layers-next.png` city work-map scene were inspected. Tests cover all
  eight direction flags, atlas crop boundaries, sheet dimensions, and PCX/PNG
  manifest overrides.
- The latest seed-42 desktop self-test passes 271/278 checks. The previously
  failing civil-war capture, diplomacy/armistice, Farmland, Airstrip, and
  Airbase scenarios now pass. Seven unrelated gameplay checks remain failing:
  `achievements`, `partisan_resistance`, `surface_warships`, `wonder_output`,
  `obsolete_wonders`, `paratrooper_requirements`, and `ai_domestic_buildings`.
  Report: `/tmp/civcraft-playtest-civ3-eight-way-final/report.json`.

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
