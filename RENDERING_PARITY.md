# Rendering parity — 2026-11-06

Historical report: the Amplio2 implementation and validation described below
have since been retired. Current map rendering uses Civ III sheets and vector
overlays; horizontal wrap with bounded poles is the only topology, and older
save formats are unsupported. The historical WrapXY notes below describe a
removed path and are not supported behavior or current validation guidance.

This work addresses the five map-rendering gaps from the rendering audit. It
uses the local native Freeciv client as the source for Amplio2 sprite selection,
layering and remembered information, and keeps CivCraft's local single-player
simulation and Macroquad renderer. It does not introduce a server or claim
complete Freeciv UI parity.

## Observed map state

`src/sim/observed_map.rs` records tile terrain and extras, and public city
appearance (name, owner, size, art cell, walls, occupancy and disorder) when
observed. Hidden changes do not replace a player's last observation. Seeing an
empty former city tile removes the remembered city; hidden destruction or
capture does not. World rendering, city terrain and the minimap use the same
observed data. Ordinary city production/food details remain private.

Map trades and shared vision transfer the giver's recorded observations,
including an observation of an empty former city site, rather than inspecting
current hidden world data. Observation and rendering do not consume RNG.

Save V14 persists these observations. V1–13 remain readable: recorded route
terrain, river, road and rail facts are preserved; unavailable historical visual
facts are reconstructed once on migration. Old files cannot recover artwork
history that was never saved.

Reference: `server/maphand.c` private player tiles and map sharing;
`server/citytools.c:2781` `update_dumb_city` and `reality_check_city`.

## Connected artwork and blending

`src/client/terrain.rs` selects forest/jungle and hill/mountain variants from
known visual-cardinal neighbors. Forest and jungle share a match class; hills
and mountains share another. Visual matching follows the four-cardinal Amplio2
art independently of the game's six movement directions. Unknown and off-map
neighbors repeat the current terrain. Terrain never consumes gameplay RNG.

Land boundaries use the original tile dither mask with reference neighbor
quadrants, generated once when assets load. Ocean ice cliffs and lake shores
use the original water sheet's corner cells. Dedicated artwork and sprite
specifications remain credited in `CREDITS.md`.

Reference: `data/amplio2.tilespec:195–335`, `amplio2/{terrain1,terrain2,hills,mountains,water}.spec`;
`client/tilespec.c:5362–5710` terrain blending, neighbor matching and layer selection.

## Painter passes

`src/client/map_layers.rs` defines shared map-wide passes: base tiles, blending,
terrain overlays, Water (irrigation then rivers), Roads, Special1 (resources,
mines, villages and base backgrounds), Grid/borders, City, Special2 (airfields,
pollution and fallout), followed by fog, units/base foregrounds, city bars,
focus/playback and go-to lines. The city work map uses the same pre-fog passes.
Roads complete before rail segments at mixed junctions. Road, river and border
neighbors stop at the polar rendering edges. Mountain anchors include the
reference full-tile offset without adding the six-pixel adjustment twice.
Fields are suppressed under cities, as in the reference. Each layer finishes
across visible tiles before the next layer begins.

Reference: `data/amplio2.tilespec:108–139`; `client/tilespec.c:5240–5275`,
`:6130–6490`; `client/mapview_common.c:1803–1847`.

## Route preview

Go-to mode previews the hovered destination using the exact observed-map search
used by the GoTo command. Cyan segments, original Amplio2 destination and
exhausted-movement markers, and zero-based arrival turn digits show the planned
route. Movement fragments, roads, already-depleted units, impassable destinations
and visible transport boarding use existing movement rules. Lines are clipped
to the world viewport and split at horizontal seams; the viewport follows the campaign wrap mode. The observed movement-cost helper was aligned with actual execution for river
discounts, hostile infrastructure, wheeled terrain access and transport boarding;
regression tests compare preview positions and movement points with real commands.
Route markers and digits are copied into a transparent padded atlas so source
sheet grid lines cannot bleed at compact/fractional zoom. Clicking still executes
the ordinary GoTo command.
There are no newly invented persistent movement orders.

Reference: `client/goto.c:1250–1340`, `client/mapview_common.c:2488`,
`amplio2/terrain1.spec` path markers, `client/tilespec.c:3547` digit fallback.

## Verification

Unit tests check reference sprite tags for connected terrain and lake/ice cells,
blend-atlas pixels, observed terrain/extras/city changes, map transfer, save
round trips and compatibility, and route/command equivalence with unchanged RNG.

The native gallery adds `terrain-blending`, `map-layers`, `remembered-map` and
`goto-preview`. The layer case opens the actual city work-map view; the memory
case returns vision to show changed artwork; the route case executes the real
GoTo command. Full gallery runs and the desktop gameplay suite complement these
fixtures. Captures must also be visually inspected; pixel differences alone do
not prove fidelity.

Final validation: 556/556 Rust tests, 278/278 native gameplay checks, and
33/33 gallery scenes at both 1280x720 and 800x600 (45 desktop and 46 compact
captures). The terrain, layer/city-map, remembered-city before/after and route
captures were visually inspected. The compact route atlas was padded after
inspection caught source grid-line bleed. Formatting and diff checks pass.

Wrap settings were subsequently made configurable: the default `--wrap x` has
bounded poles, and `--wrap xy` displays the torus continuously across both seams.
The earlier horizontal-only rendering boundary is no longer applied to torus
campaigns. See README.md and AGENTS.md for the CLI and geometry contract.

## Ocean boundaries and native camera limits

The map and city-map backing fill is now the reference `mapview_unknown` color,
black (`data/misc/colors.tilespec:4–6`). The WrapX world view adds decorative
ocean rows outside the playable map; city work maps keep their ordinary off-map backing. Actual generated
glacier/tundra tiles use the normal terrain passes.

`src/client/map_view.rs` follows native
`client/mapview_common.c::get_mapview_scroll_window` (1110–1192) and
`calc_mapview_origin` (980–998) for WrapXY. WrapX adds two visual ocean rows at
each bounded edge and extends the camera frame through that blue border. Its
real first and last tile rows remain fully visible. The decorative outermost rows are
cut at their widest point, with a one-pixel sampling guard hiding the
staggered outline. A height-dependent minimum zoom keeps that padded extent
filling the viewport. The decorative rows change no dimensions or gameplay
bounds. WrapX generation separately removes polar land in favor of ocean; see AGENTS.md.

The map center, input inverse projection and minimap viewport use the viewport
midpoint, as in `center_tile_mapcanvas` (1239–1257).

WrapXY normalizes the GUI origin modulo the map's vertical pixel period instead
of clipping it. Map tiles, neighbor-dependent art and corner knowledge wrap
through that seam, without off-map terrain or polar fog caps. In the WrapX
world view, decorative ocean and corner fog inherit the closest
real boundary row's visibility, so unknown land is not revealed and fully visible
water has no dark join. The city work map retains reference missing-corner fog
semantics (`tilespec.c:5425–5430`): off-map members count as remembered.

Unit checks compare torus camera results with an independent literal
translation of the native absolute GUI calculations. Cylinder checks verify
that visual water covers both edges while real boundary rows remain visible
across
fractional zooms and oversized viewports. The `north-pole` / `south-pole`
gallery scenes check extended anchors, input hit testing, blocked WrapX
panning, zoom-out coverage and continuous
WrapXY panning. WrapX `-next` captures show its minimum permitted zoom.
Existing `wrap-seam` checks execute real unit movement at the boundary.
The decorative extension affects presentation only. The WrapX terrain policy
is described in AGENTS.md; neither change consumes campaign RNG.

## Ice transition styling

Ice-to-land boundaries deliberately use a custom narrow stipple after visual
comparison with the reference one-way blend. A six-source-pixel fade controls
fixed spatial noise, clipped inside the tile diamond. This uses no gameplay
RNG. Other terrain pairs retain the native blend mask and eligibility rules.

The glacier's transparent raster edge is backed with matching ice artwork
instead of a flat minimap color, avoiding a blue-gray fringe. The
`ice-transitions` gallery scene provides normal and fractional-zoom captures.

## Polar shoreline coverage

Ocean corner sprites are geometrically clipped to their own tile diamond,
preventing expanded alpha masks from painting onto neighboring ice. The bright
ice-cliff lip extends one source pixel upward into transparent pixels to meet
the ice surface without a blue line. Original opaque cliff pixels and shadow
placement are preserved; lake cells are unchanged. This small coverage fix is
validated against the actual generated seed-42 polar shoreline.
