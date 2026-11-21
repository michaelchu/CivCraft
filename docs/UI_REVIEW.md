# UI structure and visual review

## Current renderer contract

The supported map path is the Civ III renderer with horizontal wrap and bounded
north/south edges. Use `--ui-review` to capture the north-pole, south-pole,
terrain, river, and wrap-seam scenes at 1280×720, with 1600×900 for wide
layouts. The game window cannot be smaller than 1280×720 or leave 16:9, so
there is no compact size to review. Inspect the
actual PNGs for continuous ocean backing at both poles, connected terrain and
river art, and clean horizontal viewport seams. WrapXY scenes and prior
Amplio2-atlas checks are retired.

The detailed notes below record historical implementation and review work.
Their Amplio2 atlas claims, old save compatibility checks, and former WrapXY
comparisons are not current behavior or validation requirements. Entries that
mention compact or 800×600 captures predate the 1280×720 window minimum and are
kept as records only.

The client uses Macroquad's immediate-mode renderer. View files own their screen-specific content; `src/client/ui.rs` owns shared pixel-measured text fitting, panel framing, dialog text bounds, gutters, grid sizing and pagination. Buttons and meters remain shared primitives in `src/client/mod.rs`.

This is deliberately a small shared UI layer rather than a retained widget framework. Screen-specific game state and graph rendering remain in their existing modules. Some view coordinates still use local offsets; the UI is more consistent but is not completely free of duplicated layout code.

`ClientSimulation` caches the most recently requested unit action view and
invalidates it before commands, smoke-test turns, and review-fixture mutations.
The last city-manager projection and goto preview share this invalidation;
the goto key includes the actor and destination and also caches unreachable
results. These projections use shared immutable values so repeated frames do
not clone their production catalogs or route points. Notification screens
borrow the inbox, and stack labels use identity queries instead of action rules.
Selection checks use lightweight identity queries. The world painter filters
city and unit positions before calculating their display projections. Minimap
terrain and fog use a nearest-filtered texture refreshed on the same client
revision; city playback markers and the camera outline remain dynamic. New
client mutation paths must invalidate these cached views.

Post-turn unit selection scans eligible IDs once while preserving ascending-ID
cycling and the waiting-list fallback. Playback retains its first queued unit
and city overrides until the queue changes; actor iteration and inactive sound
filtering avoid temporary lists. Initial unit-art loading builds the first team
palette texture from the same FLC decode as the base art. Other team palettes
remain lazy, and decoded frames are not retained after texture preparation.
Atlas packing writes directly into the final images; baked team atlases omit
the unused tint image. Pixel comparisons cover palette animation, team colors,
smoke, shadows, direction rows, and frame rows spanning the texture-width limit.

Turn execution owns the live simulation on a worker thread. The UI keeps one
frozen campaign copy and accepts no gameplay commands until completion; audio
controls and window rendering remain available. UI notices arriving during a
turn are queued and appended after completion, preserving notification IDs.
A completed turn projects its
visual events and encodes optional autosave data on that worker. The UI prepares
animation metadata in batches of at most 64 events with a two-millisecond soft
budget, then installs the campaign and presentation queue together. One event
always makes progress even if it exceeds the budget. The frozen copy itself is
created on the UI thread, so very large campaigns still have a copy cost.

Autosave disk writes use a separate ordered worker and the existing temporary
file, flush, and atomic-rename protocol. Its queue is bounded; if disk writes
fall behind, only the latest unsent snapshot is retained. Shutdown flushes that
snapshot. Unit audio uses another bounded worker for AMB parsing, component
lookup, and waveform decoding; successes and failures are cached. Raw waveform
buffers are released after device registration while cue metadata stays cached.
The device
thread registers at most one new sound per frame and drops muted requests.
Cues more than 250 milliseconds late expire to prevent catch-up bursts after
cold loading or a backgrounded window.
Smoke-test turns use the same asynchronous action path. UI review pauses its
scripted frame counter while a turn is processing, then checks its result.
UI-review action timings measure dispatch cost; full turn latency also includes
the wait for worker completion and presentation preparation.

Native macOS release measurements on 2026-11-20 used seed 42, an 80×80 small
map, a human-founded Rome, music and sound effects enabled, and 12 early turns
per autosave setting. After startup warmup, median UI handoff was 0.28 ms with
autosave off and 0.27 ms with it on. Median time through the first `next_frame`
after turn completion was 66.4/66.8 ms; this is a frame-loop measurement, not a
GPU scanout timestamp. Median frame intervals were 16.7 ms in both runs, with
maxima of 29.3/19.4 ms. Background save writes had a 5.3 ms median, all 12
succeeded, and the final save reloaded and advanced successfully. A separate
artificially delayed turn still allowed the audio panel to open. These are
early-game responsiveness checks, not late-game or bulk-animation benchmarks.
Temporary timing probes and the delay were removed from the normal build.
The rebuilt native gallery passed all 49 scenes and 177 checks. Fixture setup
now clears the complete selected-unit group, preventing the idle-garrison scene
from restoring a previously selected unit on its first draw.

## Reviewed surfaces

| Surface | Cleanup |
| --- | --- |
| Control buttons | There is no top bar. Government, diplomacy, espionage, research, spaceship and audio icons float in the upper right over the map. The science, tax and luxury rate sliders live in the Government panel. |
| Map overlays | The map fills the whole window. The minimap sits in the lower left. The context card and End Turn sit in the lower right, and the card always shows year, government, gold and research progress. Recent messages sit below the floating buttons. |
| Action strip | One centered row of icons. Hover shows the action and its shortcut when available. More opens the next page of actions. |
| City manager | Wider responsive modal; separate name, forecast, progress and specialist rows; production grid adapts rows and columns to available space. |
| Production cards | Names, cost and status fit their cards; pagination uses the actual grid capacity. |
| Worklist / worked tiles | Queue labels leave space for editing controls. Worked tiles use an isometric map with shared world terrain, improvements and city artwork; surrounding tiles are fogged, with diamond-shaped worker hit targets. |
| Government | Common frame, bounded prose, existing two-column government choices. |
| Diplomacy | The negotiation meeting centers the leader portrait and deal table, with tradable items on each civilization's side; map and city offers remain available in narrower windows. |
| City deals / treaties | Names stay clear of purchase controls; treaty columns fit independently and reserve footer space. |
| Spy / investigation | Common frame, bounded titles and content, shared Previous/Next footer with page count. |
| Research tree | Pixel-bounded summaries and details, consistent tab gutters; tree drawing is unchanged. |
| Spaceship | Common frame and bounded prose; slot labels use supported font glyphs. |

The city manager now captures modal input and removes underlying HUD hitboxes. This prevents clicks on covered HUD controls.

## Reproduce visual QA

Visual QA requires a complete Civ III installation. `CIVCRAFT_SOURCE_DIR` must
point to its root and that root must contain `Conquests/Art/Terrain`. If the
variable is unset or the directory is unavailable, do not run `--ui-review` or
baseline comparisons in that environment. Run the portable headless and Rust
tests instead and report visual review as skipped, not passed. Baselines made
with missing, fallback, generated, or partial assets are not valid substitutes.
The executable also rejects a missing or invalid source directory, but agents
should perform the preflight below rather than launching a review expected to
fail during startup.

From the app directory:

```sh
export CIVCRAFT_SOURCE_DIR="$HOME/Documents/projects/CivCraft/source"
test -d "$CIVCRAFT_SOURCE_DIR/Conquests/Art/Terrain"
cargo build --offline --release
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-ui-review --window-size 1280x720
```

Compare a run against an explicit baseline gallery with the perceptual hybrid
metric from `image-compare`:

```sh
./target/release/civcraft --seed 42 \
  --ui-review /tmp/civcraft-ui-review-current \
  --ui-baseline /tmp/civcraft-ui-review-baseline \
  --ui-min-similarity 0.99 --window-size 1280x720
```

Baseline files are never updated automatically. Every captured filename must
exist in the baseline directory with identical dimensions. Similarity scores
are recorded as `baseline_similarity` in `report.json`; a score below the
threshold fails the run. Any non-identical image writes a `*-diff.png`
visualization beside the current capture. The default threshold is `0.99` when
`--ui-baseline` is used.

The gallery covers 49 scenarios, including the campaign intro and three campaign flows built from an ordinary seeded new game. Those flows found a city, change worker allocation and restore it, change taxes, and end a turn through the real UI hit-testing and action-dispatch functions. The `diplomacy-negotiation` fixture first offers peace without compensation and verifies the rejection, then adds gold, resubmits, and checks that peace is signed and gold deducted. Other scenarios provide controlled examples for developed cities, diplomacy, first contact and war status, leader avatars, research, touching national borders, and six-direction rivers with lake outlets and fog; `report.json` identifies each scene as `campaign` or `fixture`.

The `world` scene reveals every generated tile and centers the map at the minimum zoom. Use a wide window (1600×900) to inspect landmass shape, biome bands, and polar edges across generation settings. It does not change normal game visibility.

For a quick on-demand check, select a scene by name:

```sh
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-borders --ui-scene touching-borders
./target/release/civcraft --seed 1337 --ui-review /tmp/civcraft-campaign --ui-scene campaign-after-turn
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-first-contact --ui-scene first-contact
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-leader-diplomacy --ui-scene leader-diplomacy
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-negotiation --ui-scene diplomacy-negotiation
```

Every scene produces its primary screenshot. A `-next.png` is saved only after a control changes the page or campaign state; the runner verifies that the captured pixels changed too. Managed screenshots from an earlier run are cleared before capture, so old pages cannot masquerade as new results. Autosaves are disabled and, except for the go-to hover scenario, the gallery parks its virtual cursor outside the UI to keep captures independent of the OS pointer. Normal interactive play keeps its actual mouse hover behavior.

`report.json` lists screenshots, assertions, optional baseline similarities,
CPU draw-submission timings, setup/capture timings and border-cache statistics.
Failed checks, baseline mismatches, missing controls, out-of-window hitboxes,
unchanged next-page captures and report-write failures return a nonzero exit
status. `--ui-review` runs separately from `--screenshot` to keep their state
mutations independent. Missing automation-flag arguments fail
rather than starting interactive play.

Repeat at 1600x900 to inspect the wide layout. `--window-size` accepts only
16:9 sizes of at least 1280x720; any other value silently falls back to
1280x720, so check the PNG dimensions when a run must use a specific size. Run the Rust test suite separately:

```sh
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

Screenshots still need visual inspection: bounds and pixel fingerprints alone cannot prove every UI behavior. The window cannot be dragged below 1280x720 or off 16:9; maximized or snapped windows can have other sizes and are outside the tested range. Parity implementation remains paused during this UI work.

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
  midway (or at the end of the casualty's death animation); `-next.png` captures the final state after playback finishes.

Normal gameplay uses Freeciv's default 30 ms movement. Combat exchanges use
resolved Civ III source clip timings and presentation pacing overrides, holding
shorter clips at their final frame until both participants finish. Each exchange
has a fresh animation clock; damage is displayed after that exchange. Death and
victory receive a separate source-timed phase. Ordinary casualties use their
unit death artwork alone. Only bombardment events with the explicit Bombard
Effects flag receive a separate five-frame impact overlay (20 ms per frame). Missing defend/secondary attack clips and sounds fall back
to the primary attack. Playback is a bounded,
non-persistent observer of simulation movement/combat/bombardment results.
It never makes gameplay decisions or consumes random draws. Native tests
compare complete saved campaign bytes and the full gameplay RNG with the
observer enabled and disabled. The gallery's fixed progress and selection
frame are opt-in visual QA controls, not alternate game logic.

City appearance uses the imported civ2civ3 style requirements and city image /
visible walls effects plus Amplio2 sprite tags. The six historical civilizations
have explicit initial themes; there is no Freeciv nation-selection feature.
Their banners use the existing faction colors rather than unrelated national
flags.


## Fog transitions

`--ui-scene fog-transitions` captures visible/remembered/unknown boundaries
using the original Civ III `Art/Terrain/FogOfWar.pcx` 9×9 alpha atlas and the
four-neighbor mapping used by OpenCiv3. Unknown, remembered, and visible map to
atlas values 0, 1, and 2; north/west states select columns, and east/south
states select rows. The all-visible cell is skipped because it contains
alignment pixels. The second capture follows a real Move command and checks
that simulation visibility changed. The same corner pass masks irrelevant
tiles in the city worked-tile view. Full-world gallery runs also exercise
generated maps, rivers, city masks, horizontal wrapping and polar limits. The
renderer draws each lattice corner once, before units and labels; remembered
terrain and borders no longer apply a second tint. Hovering a known map tile
temporarily renders it as active, matching OpenCiv3's `TileInfo.targetTile`
behavior without changing simulation visibility. The fog image uses the
source palette's inverse-red alpha, transparent palette indices 254/255,
nearest-neighbor sampling, and a 0.999-cell source crop. CivCraft remaps the
alpha using `FOG_ALPHA_GAMMA` (currently 1.8; 1.0 preserves source opacity) to
tighten the edge toward the original game's appearance. Cells containing remembered
corners use `REMEMBERED_FOG_ALPHA_GAMMA` (1.2) to make explored terrain outside
current sight visibly darker. Fully visible cells remain skipped, and cells
containing only unknown/visible corners retain the 1.8 curve. These code-level
values are the tuning points; there is no runtime control. The fog pass follows
map art and grid, then precedes units and labels, matching
[OpenCiv3's FogOfWarLayer](https://github.com/C7-Game/OpenCiv3/blob/Development/C7/Map/FogOfWarLayer.cs)
and its [PCX alpha decoder](https://github.com/C7-Game/OpenCiv3/blob/Development/C7/Textures/PCXToGodot.cs).
OpenCiv3's terrain base pass also draws neighboring base sprites before fog.
CivCraft draws a one-tile unseen terrain fringe around the known boundary so
the atlas can fade it into black; deeper unexplored terrain remains hidden.

Visual check on 2026-11-09: the `fog-transitions` scene was captured and
visually inspected at seed 42, 1280×720, 800×600, and 1600×900. All five
report checks passed at each size, including all three knowledge states and
the post-move visibility change. The fixture uses flat grassland and one unit
to isolate the edge. Inspection confirmed the sharp perimeter came from
omitting unknown neighbor terrain beneath the fog atlas and filling those
cells with opaque black diamonds. After adding the OpenCiv3-style terrain
fringe and letting the atlas mask it, the remembered area fades into the black
unexplored boundary. The source atlas has 16 alpha levels (including
transparent and opaque) and does not ship with a separate Dither sprite;
OpenCiv3 applies its alpha atlas directly. CivCraft's gamma curve tightens
that ramp without adding a separate dither layer. The remaining differences
from the supplied Civ III screenshot are the flat diagnostic map and CivCraft
interface.

### Rendering gap scenarios

`terrain-blending` shows mixed connected forest/jungle and hill/mountain ranges,
land dither transitions and ice cliffs. `map-layers` shows a river/road crossing
and pollution over a city, then opens its city work map. `remembered-map` freezes
a foreign city's former name, walls, style and terrain until vision returns.
`goto-preview` positions the shared test pointer over a multi-turn destination
and then executes the real GoTo command. The current renderer contract is
summarized at the top of this document.

### Wrap and polar checks

`north-pole` and `south-pole` reveal the actual generated terrain and focus the
viewport at its northern/southern limits. Campaign maps wrap horizontally and
have bounded polar edges. Review the generated north/south screenshots and
horizontal viewport seams after rendering changes.

### Native polar backing and camera

The polar scenes check the bounded north/south camera and input projection.
World and city work maps clear to reference black. Camera geometry tests cover
subpixel zoom and oversized viewports. Current map policy and screenshot checks
are summarized at the top of this document.

The current rendering cleanup removes the former two-axis wrap path. Validate
polar limits and horizontal seam screenshots for the single supported map
topology.

### Terrain atlas and seam checks

`terrain-seams-small`, `terrain-seams-default`, and `terrain-seams-large`
show all terrain types in adjacent patches at 0.625×, 0.82×, and 1.35× zoom.
Each captures a fractional camera position and a second position after panning.
Inspect uniform water interiors, coastlines, lake/ice corners, and connected
forest/jungle and hill/mountain edges for straight grid lines or gaps. Run at
1280×720 and 1600×900 when changing terrain sampling. The gallery
assertions check execution and bounds; screenshots still require visual
inspection.

Land, connected overlays, and lake/ice cells use isolated padded atlas cells.
Lake and ocean mask edges have textured backing clipped to the tile diamond,
using the matching water artwork. Ocean masks overlap their raster edges by one texel to prevent coastline gaps
at fractional zoom. A reusable mesh avoids per-tile allocations.
This replaces the flat water backing, which could show dark lake diagonals.

The older terrain seam capture counts describe a superseded renderer and should
not be used as current validation evidence. Capture the current Civ III scenes
at 1280×720 and 1600×900.

### Ice transitions

`ice-transitions` surrounds an ice patch with desert, grassland, plains,
and swamp. Ice-to-land boundaries use a custom six-source-pixel stipple,
with fixed spatial noise and no gameplay RNG draws. This deliberately differs
from the native one-way `t.blend.arctic`/`t.dither_tile` blend. Other terrain
pairs, including ice-to-water boundaries, retain their reference behavior.

The stipple stays inside each tile's diamond. Transparent glacier raster edges
have matching ice artwork underneath, preventing a blue-gray fallback fringe.
`ice-transitions-next.png` checks fractional zoom and camera panning.

Polar shoreline checks also inspect the actual seed-42 `north-pole` capture.
Ocean corner drawing is clipped to shared diamond geometry so expanded alpha
masks cannot paint blue onto neighboring ice. Ice-cliff sprites keep their
original placement and shadows; only their bright lip extends upward by one
source pixel into transparency. Lake cells are unchanged.

### Native map dimensions and polar generation

Campaigns use the Civ III 36×48 horizontal map topology with bounded polar
edges. The former WrapXY topology is removed from the supported game paths.

### WrapX ocean boundaries

WrapX keeps the polar edge rows ocean and clears gameplay extras from them.
Two additional ocean rows extend the visual water beyond each playable edge;
they are render-only and do not enter simulation or saves. The `north-pole` /
`south-pole` scenes show the ocean edge; `-next` captures show minimum zoom.

Gallery checks cover the extended camera anchors, actual boundary-row input,
bounded panning and zoom-out coverage. See AGENTS.md for map-generation
validation guidance and the current renderer contract at the top of this document.

The `religion` gallery scene opens a prepared faith in the Religion management
panel. Its staged interaction cycles declared faith, selects Devout, confirms
through normal hit testing, and checks the resulting declaration and cooldown.
As with other scenes, capture/comparison requires the complete Civ III assets
and an explicit baseline; do not overwrite the baseline implicitly.

### Borders, fog and city discovery

`--ui-scene borders-fog-cities` stages Visible Rome, Remembered Babylon and
an undiscovered AI city beyond the black fog. Two territories cross the
visible/remembered/unknown boundary. Six scene checks require the visible and
remembered city views, reject the unknown city view, and verify all three tile
knowledge states. Inspect the PNG for connected borders fading beneath the fog
and city sprites/labels in known territory. This is an isolated rendering fixture;
shared-vision pact transitions are covered by the simulation regression.

`--ui-scene ai-city-garrisons` shows idle, fortified and sentry AI defenders
above cities in current sight, plus a remembered city whose defender stays hidden.
The human map displays these defenders independently of military-access treaties.

`--ui-scene combat-animation-frames` issues a real combat command and captures
the first frame of the first attack, then 65% through that same exchange.
HP remains at its pre-exchange values in both captures; the screenshot runner
requires changed pixels. The legacy `city-combat-explosion` scene name now samples the final death
animation without an explosion overlay, independently of combat length. Combat duration follows the resolved INI
clip timings, including per-action presentation pacing overrides.

Combat clips default to a 2.0 duration multiplier in
`data/presentation/unit_animation_pacing.toml`, giving a source half-second
attack one second to play. The same setting drives animation frames and round/
result durations, so slower motion also extends the battle. Explicit per-unit,
per-action pacing overrides still take precedence.
