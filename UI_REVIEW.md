# UI structure and visual review

The client uses Macroquad's immediate-mode renderer. View files own their screen-specific content; `src/client/ui.rs` owns shared pixel-measured text fitting, panel framing, dialog text bounds, gutters, grid sizing and pagination. Buttons and meters remain shared primitives in `src/client/mod.rs`.

This is deliberately a small shared UI layer rather than a retained widget framework. Screen-specific game state and graph rendering remain in their existing modules. Some view coordinates still use local offsets; the UI is more consistent but is not completely free of duplicated layout code.

## Reviewed surfaces

| Surface | Cleanup |
| --- | --- |
| Top bar | Distinct turn, government, diplomacy, treasury and research regions; measured text bounds prevent collisions. |
| Sidebar | Seed gets its own row; context text stays inside the column; compact windows use tighter context spacing; rate controls share equal gutters. |
| Action strip | Two rows and a More control expose overflow actions instead of dropping them. |
| City manager | Wider responsive modal; separate name, forecast, progress and specialist rows; production grid adapts rows and columns to available space. |
| Production cards | Names, cost and status fit their cards; pagination uses the actual grid capacity. |
| Worklist / worked tiles | Queue labels leave space for editing controls. Worked tiles use an isometric map with shared world terrain, improvements and city artwork; surrounding tiles are fogged, with diamond-shaped worker hit targets. |
| Government | Common frame, bounded prose, existing two-column government choices. |
| Diplomacy | Rival details sit above wrapping action rows; map and city offers remain available in narrower windows. |
| City deals / treaties | Names stay clear of purchase controls; treaty columns fit independently and reserve footer space. |
| Spy / investigation | Common frame, bounded titles and content, shared Previous/Next footer with page count. |
| Research tree | Pixel-bounded summaries and details, consistent tab gutters; tree drawing is unchanged. |
| Spaceship | Common frame and bounded prose; slot labels use supported font glyphs. |

The city manager now captures modal input and removes underlying HUD hitboxes. This prevents clicks on covered HUD controls.

## Reproduce visual QA

From the app directory:

```sh
cargo build --offline --release
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-ui-review --window-size 1280x720
```

The gallery covers 39 scenarios, including three campaign flows built from an ordinary seeded new game. Those flows found a city, change worker allocation and restore it, change taxes, and end a turn through the real UI hit-testing and action-dispatch functions. The other scenarios provide controlled examples for developed cities, diplomacy, research, touching national borders, and six-direction rivers with lake outlets and fog; `report.json` identifies each scene as `campaign` or `fixture`.

For a quick on-demand check, select a scene by name:

```sh
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-borders --ui-scene touching-borders
./target/release/civcraft --seed 1337 --ui-review /tmp/civcraft-campaign --ui-scene campaign-after-turn
```

Every scene produces its primary screenshot. A `-next.png` is saved only after a control changes the page or campaign state; the runner verifies that the captured pixels changed too. Managed screenshots from an earlier run are cleared before capture, so old pages cannot masquerade as new results. Autosaves are disabled and, except for the go-to hover scenario, the gallery parks its virtual cursor outside the UI to keep captures independent of the OS pointer. Normal interactive play keeps its actual mouse hover behavior.

`report.json` lists screenshots, assertions, CPU draw-submission timings, setup/capture timings and border-cache statistics. Failed checks, missing controls, out-of-window hitboxes, unchanged next-page captures and report-write failures return a nonzero exit status. `--ui-review` runs separately from `--self-test` and `--screenshot` to keep their state mutations independent. Missing automation-flag arguments fail rather than starting interactive play.

Repeat at 800x600 and 1600x900 to inspect compact and wide layouts. Run the desktop interaction suite separately:

```sh
./target/release/civcraft --seed 42 --self-test /tmp/civcraft-ui-interactions.json
cargo test --offline
```

## Optimization and fidelity

- Generate the campaign once, build the staged fixture once, and clone immutable baselines for independent scenarios. Clone tests verify normal command results and the RNG stream after advancing a turn.
- Reuse the immutable technology layout across games. The current technology catalog is fixed; player research state and highlighting stay live.
- Cull world rows before collecting tiles and reuse each visible projection for both terrain passes. Native row order preserves the original painter order, including horizontal wrapping and polar clipping. Cities and unit stacks are indexed once per frame, with the existing selection priority preserved.
- Cache border geometry by exact screen vertices and width. The cache is bounded to 4,096 entries; camera/zoom/resize changes use different keys. Ownership, visibility and colors remain current; terrain and city artwork deliberately retain their last observation outside sight.
- Keep the production renderer and simulation commands in the test path. Gallery fixtures complement the desktop gameplay suite; scripted control clicks do not inject OS keyboard/mouse events.

For the original 15 scenarios at 1280x720, two local profiling runs reduced setup from about 31–32 ms to 2.2–2.4 ms and captures from 30 to 18. The comparison run spent about 662 ms capturing/encoding PNGs before and 482 ms afterward, including the new pixel fingerprints. The optimization pass added three campaign flows, bringing that gallery to 23 captures at this size; the subsequent river, garrison and map-presentation cases bring the gallery at that point to 38. CPU draw submission averaged about 0.57 ms in both versions; these small scenes do not demonstrate a substantial FPS gain. Timings measure CPU submission, not GPU frame time.

## Validation on 2026-11-05

- 512 Rust tests and 278 native desktop interaction checks passed.
- During the preceding optimization pass, all 18 retained standard-size screenshots were byte-identical to the original renderer in two comparisons. River artwork was intentionally changed afterward. The three additional campaign flows passed their state and hit-testing assertions.
- All compact-window flows and control bounds passed. Compact screenshots have small text-sampling differences across processes; repeating the unmodified baseline also reproduces those differences. For example, the baseline-to-optimized world view differed at 448 of 1,920,000 pixels, while two unmodified baseline runs differed at 715 pixels, concentrated in text.
- The campaign turn flow also passed with seed 1337 using single-scene capture.
- Formatting and whitespace checks passed.

Screenshots still need visual inspection: bounds and pixel fingerprints alone cannot prove every UI behavior. Windows smaller than 800x600 are outside the tested range. Parity implementation remains paused during this UI work.

### Isometric worked-tile view

The city map reuses the world's terrain and improvement passes. It fits the city radius into the available panel space, clips terrain overhangs, and fogs irrelevant or unexplored tiles. Gold outlines show the city center and worked tiles, with each shared edge drawn once at a consistent thickness; tile yield labels remain visible. Click targets follow each diamond rather than its overlapping bounding rectangle. Preview rendering does not move or zoom the main world camera.

Validation: 490 Rust tests and 278 desktop checks passed, including worker/specialist reassignment. Standard and compact window captures passed control-bound checks; the main-world screenshot remained pixel-identical after extracting the shared terrain pass.

Territorial borders use a thin player-colored band with a dark outline and diagonal stitching. Each filled band is clipped to its owning tile and reaches the exact shared boundary, so both colors meet without a gap or protruding line caps. The dark outline is drawn only on the terrain-facing side. The touching-borders gallery scenario demonstrates this with red and gold territories.

### Six-direction river artwork

`src/client/rivers.rs` loads Freeciv's Hexemplio river sheet, maps all 64 connection masks and six outlet tags from its original `.spec`, and adapts its 126x64 hex footprints into the existing 96x48 diamond tiles. Area sampling preserves channels at the collapsed NW/SE tips; premultiplied color accumulation avoids transparent-background fringes, and transparent atlas gutters isolate neighboring patterns when zooming. Atlas construction happens once when assets load. Each river tile then uses one textured sprite, with separate sprites for water outlets.

The hand-drawn NW/SE blue-line fallback is removed. River flags, generation passes and random draw order are unchanged. The five-seed Freeciv map comparison still passes. Rendering tests verify all 64 source tags, all six outlets, reciprocal diagonal endpoint coverage, diamond masking and fog visibility.

Three gallery cases cover this layer:

```sh
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-rivers --ui-scene rivers-six-directions
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-rivers-fog --ui-scene rivers-fogged
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-rivers-generated --ui-scene rivers-generated
```

The first two are explicit artwork fixtures, including a six-outlet lake. `rivers-generated` reveals and focuses an actual generated river without changing terrain, extras, RNG or starting positions. Native gallery checks cover 1280x720 and 800x600.


City garrison presentation can be captured with `--ui-scene city-garrison-idle`
and `--ui-scene city-garrison-selected`. These use the same city and garrison;
only unit focus changes. Idle garrisons are hidden, focused units draw above
the city in a final focus layer. The original Amplio2 selection ring animates
at 6 Hz during gameplay and holds frame zero for deterministic gallery captures.


## City and unit presentation

- `unit-status` stages healthy/damaged units, veteran ranks, fortification,
  sentry, worker jobs, automation, exploration and stacked-unit markers.
- `city-appearance` stages small classical, walled European, industrial, and
  postmodern/disordered cities. Own-city bars include production turns and
  growth/famine; foreign-city bars exclude private stock information.
- `unit-movement`, `city-combat`, and `city-combat-explosion` stage only their initial conditions, then
  issue real Move commands. The primary capture holds actual event playback
  midway (or during the casualty explosion); `-next.png` captures the final state after playback finishes.

Normal gameplay uses Freeciv's default 30 ms movement and 10 ms per lost HP,
followed by five 20 ms explosion frames for a casualty. Playback is a bounded,
non-persistent observer of simulation movement/combat/bombardment results.
It never makes gameplay decisions or consumes random draws. Native tests
compare complete saved campaign bytes and the full gameplay RNG with the
observer enabled and disabled. The gallery's fixed progress and selection
frame are opt-in visual QA controls, not alternate game logic.

City appearance uses the imported civ2civ3 style requirements and city image /
visible walls effects plus Amplio2 sprite tags. The six historical civilizations
have explicit initial themes; there is no Freeciv nation-selection feature.
Their banners use the existing faction colors rather than unrelated national
flags. See `UI_PARITY_AUDIT.md` for source pointers and validation coverage.


## Fog transitions

`--ui-scene fog-transitions` captures visible/remembered/unknown boundaries
using the original Amplio2 corner atlas. Its second capture follows a real
Move command and checks that simulation visibility changed. The same corner
pass masks irrelevant tiles in the city worked-tile view. Full-world gallery
runs also exercise generated maps, rivers, city masks, horizontal wrapping
and polar limits. The renderer draws each lattice corner once, before units
and labels; remembered terrain and borders no longer apply a second tint.
Tests verify all 81 state-to-cell mappings, atlas alpha, horizontal wrapping
and non-wrapping polar samples. The all-visible cell is intentionally unused,
as in the reference's sprite spec. Runtime cells have one-pixel gutters,
so fractional zoom cannot sample adjacent sprites or the unused credits cell.

### Rendering gap scenarios

`terrain-blending` shows mixed connected forest/jungle and hill/mountain ranges,
land dither transitions and ice cliffs. `map-layers` shows a river/road crossing
and pollution over a city, then opens its city work map. `remembered-map` freezes
a foreign city's former name, walls, style and terrain until vision returns.
`goto-preview` positions the shared test pointer over a multi-turn destination
and then executes the real GoTo command. See [RENDERING_PARITY.md](RENDERING_PARITY.md)
for reference sources, save migration and fidelity boundaries.

### Wrap and polar checks

`north-pole` and `south-pole` reveal the actual generated terrain and focus the
viewport at its northern/southern limits. `wrap-seam` stages a unit at the top
edge and executes the same real Move command: it crosses in `--wrap xy`, and is
rejected in default `--wrap x` before an allowed move confirms input still works.
Run the gallery separately with each CLI mode; the sidebar identifies the mode.

Wrap validation: 562/562 Rust tests, including reproducible cylinder generation,
12-turn cylinder campaigns, per-mode movement/sight/path/city-radius checks,
new-save mode persistence, and unchanged torus seed oracles. Both modes passed
278/278 native gameplay checks and all 36 gallery scenes at 1280x720 and 800x600
(49 desktop / 50 compact captures per mode). Polar sprites, camera clearance,
minimap seam outlines and actual blocked/allowed crossing screenshots were
visually inspected. Formatting and whitespace checks passed.

### Native polar backing and camera

The polar scenes now assert the reference first/last normal-sprite anchor and
input inverse projection. In WrapX, attempting to pan beyond either bound keeps
the camera fixed. In WrapXY, panning across the seam continues one adjacent row
and produces an additional `-next` capture. World and city work maps clear to
reference black. Camera geometry tests also cover subpixel zoom and oversized
viewports; see RENDERING_PARITY.md for source locations and exact policy.

Polar rendering validation: 565/565 Rust tests; 278/278 native gameplay checks
in each wrap mode; all 36 gallery scenarios at both desktop and compact sizes.
WrapX produced 49 desktop / 50 compact captures; WrapXY produced 51 / 52,
including camera-seam before/after views. Pixel sampling of the native north
boundary confirmed WrapX black (0,0,0), while the same position in WrapXY
contained real terrain (0,41,136). Polar and city-map captures were visually
inspected; formatting and whitespace checks passed.

### Terrain atlas and seam checks

`terrain-seams-small`, `terrain-seams-default`, and `terrain-seams-large`
show all terrain types in adjacent patches at 0.625×, 0.82×, and 1.35× zoom.
Each captures a fractional camera position and a second position after panning.
Inspect uniform water interiors, coastlines, lake/ice corners, and connected
forest/jungle and hill/mountain edges for straight grid lines or gaps. Run at
both desktop and compact sizes and in both wrap modes when changing terrain
sampling. The gallery assertions check execution and bounds; screenshots still
require visual inspection.

Land, connected overlays, and lake/ice cells use isolated padded atlas cells.
Lake and ocean mask edges have textured backing clipped to the tile diamond,
using the matching water artwork. Ocean masks overlap their raster edges by one texel to prevent coastline gaps
at fractional zoom. A reusable mesh avoids per-tile allocations.
This replaces the flat water backing, which could show dark lake diagonals.

Terrain seam validation: 53 client tests passed. All 39 gallery scenes passed
at 1280x720 with WrapX (55 captures) and 800x600 with WrapXY (58 captures).
Terrain captures at all three zooms and after fractional panning were inspected,
along with the city work map. Lake diagonals and enlarged coastline gaps were
corrected during inspection. These checks cover the listed configurations;
visual assertions alone do not prove every possible camera/zoom combination.
