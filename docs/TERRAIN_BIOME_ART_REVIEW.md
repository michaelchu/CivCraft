# Terrain biome artwork review (2026-11-21)

## Scope and authority

Gameplay terrain placement remains the pinned Freeciv/civ2civ3 port described in
`FREECIV_GAMEPLAY_PORT_PLAN.md`. `reference/freeciv/server/generator/mapgen.c`
(`make_relief`, `make_terrains`) places terrain identities, not Civ III layers.
Civ III source artwork remains the presentation authority. Landmark terrain,
waterfalls, and Winter/European alternative packs are deferred.

## Source frame inspection

Inspected the installed PCX sheets under
`/Users/michaelchu/Documents/projects/CivCraft/source`, converted to PNG solely
for inspection in `/tmp/civcraft-terrain-evidence/`:

- `Art/Terrain/{grassland,plains,tundra} forests.pcx`: 1000×884. Cells are
  128×88. Grassland has jungle in rows 0–3 (zero based); plains and tundra
  have empty green placeholders there. Forest large frames occupy rows 4–5;
  small forest occupies rows 6–7; pines occupy rows 8–9, six columns.
- Retain grassland jungle row 2, six columns. Retain forest row 6, five columns
  for grassland/plains; use tundra pine rows 8–9, six columns. The forest frames
  on the installed biome sheets are visually similar; selecting a different
  filename does not imply every frame has different artwork.
- `Art/Terrain/Mountains-snow.pcx` and
  `Conquests/Art/Terrain/Volcanos-snow.pcx`: 512×352, sixteen 128×88 cells.
  Use the existing four-bit connected relief mask for both.

`vegetation` remains the original manifest role and its 768×616 minimum remains
compatible. Added optional roles `vegetation-plains`, `vegetation-tundra`, and
`volcano-snow`; the tundra role requires 768×880 to cover pine rows. Missing
optional biome art falls back to the original vegetation row 6 with a bounded
column; missing snow/vegetated relief falls back to ordinary relief. Explicit
invalid manifest paths retain the loader's existing error behavior.

## Generation diagnosis and change

The active Freeciv path initialized all land bases to grassland, then assigned
flat terrain bases, leaving forests and relief on that default. The alternate
OpenSimplex path already assigned relief bases independently. Its original
grassland-only forest gap and the subsequent presentation fix are documented
in `OPENSIMPLEX_TERRAIN_ART_AUDIT.md`. This is not lost
Freeciv biome metadata: Freeciv does not supply an underlying Civ III biome.

For new Freeciv maps, the adapter now assigns presentation bases after terrain
placement using its existing categorical temperature and dry-zone field:
Frozen/Cold forest and relief use Tundra; dry Temperate forest and relief use
Plains; other forest/relief use Grassland. Jungle and marsh retain Grassland.
This is explicitly a CivCraft climate-to-art heuristic, not a claim about
original Civ III snow rules. Snow takes precedence over neighboring vegetation.
No RNG calls, quotas, terrain identities, yields, resources, starting locations,
or polar-ocean rules change. Existing saved base values remain authoritative;
old campaigns are not rewritten.

## Reproduction

The ignored source-adjacent diagnostic exercises the active Freeciv generator
at canonical 36×48 dimensions, seeds 42/7/99, all Arid/Normal/Wet climates, and
Cool/Temperate/Warm temperatures (27 zero-turn maps):

```sh
cargo test --offline --release terrain_biome_census -- --ignored --nocapture \
  > /tmp/civcraft-terrain-evidence/census-after.log 2>&1
rg '^CENSUS,' /tmp/civcraft-terrain-evidence/census-after.log
```

Columns: marker, seed, climate, temperature, feature, underlying base, tile count.
The diagnostic is ignored during ordinary tests; it is an explicit release
map-generation diagnostic, not a long campaign or a debug simulation.

Visual review uses the complete installation above and seed 42. Original
captures live in `before*`; current captures in `after*`, under the evidence
directory. `terrain-blending` now stages explicit grassland/plains/tundra forests
and ordinary/snow relief. Its changed fixture is intentionally not treated as an
unchanged-baseline comparison. Baselines are never overwritten.

## Census baseline

Before the adaptation, all 7,547 feature tiles sampled across the 27 maps used
Grassland: Forest 1,979; Hills 2,838; Mountains 1,374; Marsh 916; Jungle 440.
No Volcano was generated in this sample; snowy volcano coverage therefore uses
an explicit rendering fixture and selection tests, not a procedural claim.
Default-temperature seeds alone did not contain cold forest; the Cool settings
are necessary to exercise that combination in the generation regression.

The post-change release census preserves every per-map feature count (checked
against the baseline). Forest now uses Grassland 1,878 / Plains 11 / Tundra 90;
Mountains uses Grassland 1,155 / Plains 66 / Tundra 153; Hills uses Grassland
2,595 / Plains 237 / Tundra 6. Jungle (440) and Marsh (916) remain unchanged.
`terrain-biome-census.json` retains totals and each of the 27 before/after maps.
The same diagnostic passes in release mode; the actual generation takes 0.07s.
Compilation time is separate from this zero-turn run.

## Final validation and visual limits

- `cargo test --offline --all-targets`: 1,456 library tests passed, 3 ignored;
  report integration 2/2 and headless CLI 19/19 passed. Existing Freeciv and
  CivCraft map goldens passed without updates.
- `cargo test --offline --doc`: passed (no doc test cases).
- Release desktop/headless build with `--features desktop-audio`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- Final seed-42 baseline comparisons at a requested 1280×720 window: north
  and south poles 1.0; terrain-seams-default 0.990226 / 0.99023203;
  wrap-seam 0.99921784 / 0.99918545; world 1.0. All reports/checks passed
  against the explicit 0.99 threshold. Baselines were retained separately.
- The changed `terrain-blending` fixture and an additional Cool-world capture
  passed their checks without baseline comparisons. Inspected actual final
  biome, seam, and wrap PNGs and the pixel-identical polar captures. Pines,
  ordinary and snow relief appeared with no blank crops or keyed-color blocks;
  generated poles retained continuous water backing without black gaps.
  The controlled seam fixture has deliberately artificial terrain bands;
  those coastlines are not evidence of natural-map biome quality.
- macOS Retina captures are 2560×1440 for the requested 1280×720 window;
  baseline/current dimensions match. `final-*.png` are 1280×720 inspection
  copies; original captures remain in their scene directories.
- This is a targeted seven-scene review, not the complete gallery or a
  1600×900 wide-layout run. The Cool-world screenshot is one camera viewport,
  not proof of all generated biome combinations; the census provides that
  coverage. Procedural volcanoes were absent in the sampled maps. Missing art
  behavior is covered by source-adjacent manifest/selection tests, not by a
  second incomplete-installation screenshot run.

Full local evidence: `/tmp/civcraft-terrain-evidence/`. Final scene reports are
`after/report.json`, `after-{north-pole,south-pole,wrap-seam,terrain-seams-default,world}/report.json`
and `after-cool-world/report.json`. Intermediate `after-initial*` captures are
retained for traceability but are not the final validation artifacts.

## Follow-up: hard terrain intersections

The owner's inspection of `after/terrain-blending.png` identified hard water
intersections and a rectangular-looking biome patch missed in the initial
visual review. The earlier similarity passes did not establish seamless art.

Two renderer paths contributed:

- Legacy Ice was treated as deep water in Ice/water junctions but as Tundra
  in junctions containing another land type. Adjacent cells therefore changed
  both material and shoreline interpretation. Ice now consistently uses Tundra
  land art; off-map padding is explicitly Ocean, preserving ocean-only poles.
- Native land atlases cover at most three ground types. The per-junction atlas
  choice and fallback patches could change an edge when a remote fourth biome
  changed. The renderer now feathers a narrow boundary strip from a canonical
  native cell selected only by the edge's two endpoint terrains. This is a
  CivCraft compositing correction, not recovered original-engine behavior.
  Uniform land and all-water junctions retain their previous rasterization.

The fixture, terrain generation, biome layout and source assets are unchanged
by this follow-up. Rectangular biome bands remain deliberate fixture geometry;
the correction addresses the abrupt internal material cuts, not that layout.
Source-adjacent tests cover Ice/Tundra equivalence for every neighbor type and
rotation, and remote-corner independence on all four junction edges.

Intermediate evidence is retained: `edges-fixed` shows the Ice-only correction;
`edges-stitched` shows the initial seam correction. A development-build pole
comparison initially failed at 0.98347370 because stitching also changed
ordinary water-depth junctions. That attempt was narrowed to land-containing
junctions. Its failed report remains at `edge-poles-debug/report.json`; no
baseline was replaced or approved. Development builds were used only for UI
iteration; final captures below use the release executable.

Final follow-up validation:

- `cargo test --offline --all-targets`: 1,458 library tests passed, 3 ignored;
  report integration 2/2 and CLI integration 19/19 passed.
- `cargo test --offline --doc`: passed (no doc test cases).
- Release build with desktop audio passed. Final release evidence is in
  `edge-final-*` (explicit baseline comparisons) and `edge-checked-*`
  (complete scene checks without baseline comparison), under the same evidence
  directory. The three changed scenes pass all non-baseline checks, including
  their second captures where applicable. `edge-final-wide` passes the
  terrain-blending fixture at requested 1600×900. Actual final PNGs inspected.
- At the unchanged 0.99 comparison threshold, north pole passes at 0.99795392,
  south pole at 0.99923753 and world at 0.99423093. Ocean backing remains
  continuous with no black polar gaps.
- The old terrain-blending comparison **fails** at 0.97252954,
  terrain-seams-default **fails** at 0.91999409, and wrap-seam **fails** at
  0.95579016. These fixtures include the corrected Ice/land shoreline and
  compositing, so visual inspection confirms expected material changes and
  removal of the large clipped water strips. These are failed comparisons,
  not approved replacement baselines; the originals remain untouched.
- Remaining limitations: targeted scenes rather than the full gallery; the
  synthetic biome bands still have angular layouts, and the wrap fixture's
  sharp visibility/fog boundaries are outside this terrain-material fix.

## Follow-up: soften the rectangular tundra patch

The tundra-pine patch still had a rectangular green halo after the junction
edge correction. Unsupported tundra/desert or tundra/plains combinations used
XTGC's grass proxy, then patched the other biome into geometrically clipped
regions. A softer edge alone could not remove that intervening grass.

Land-only junctions containing tundra and desert/plains now blend native pure
biome texture cells directly, using normalized smooth weights for all four
corners. This removes the proxy halo and widens the material fade without
altering terrain layout, vegetation sprites, snow-volcano artwork, simulation,
map generation or RNG. Junctions with water retain the existing shoreline path.
This is a CivCraft fallback compositor for combinations absent from the native
atlases, not a recovered Civ III algorithm. The existing shared-edge correction
continues to use canonical endpoint pairs.

A source-adjacent regression uses a conspicuous green sentinel for proxy art
and checks tundra/desert and tundra/plains in all four rotations: no proxy
material may enter the result, opacity is preserved, and each transition must
contain a continuous range of mixed values rather than a hard cut. The
fixture is unchanged. Evidence remains under `/tmp/civcraft-terrain-evidence/`.

Final softening validation:

- `cargo test --offline --all-targets`: 1,459 library tests passed, 3 ignored;
  report integration 2/2 and CLI integration 19/19 passed. Doc tests passed.
- Release desktop build passed. Final captures are `soft-blend-final-*`,
  compared to the preceding `edge-checked-*` / `edge-final-*` captures at the
  unchanged 0.99 threshold. North/south poles and world are pixel-identical
  (similarity 1.0); wrap-seam passes at 0.99979469 / 0.99957740.
- Terrain-blending comparison fails at 0.98421278 (1280×720) and 0.98972599
  (1600×900); the repeated terrain-seams-default comparison fails at
  0.98776490. These are retained failures against the earlier material edges,
  not replacement baselines. All non-baseline checks pass; the complete
  repeated-seam run is in `soft-blend-checked-seams` without a baseline.
- Inspected final release PNGs at both sizes and the repeated seam capture.
  The pine patch loses its green rectangular halo and transitions directly
  into the surrounding sand. The fixture's rectangular biome arrangement
  remains; this change softens the rendered materials rather than moving
  tiles or claiming exact native transition parity.
- `soft-blend-before-after.png` is a labeled crop of original captures,
  before on the left and after on the right. Full-resolution final image:
  `soft-blend-final-terrain-blending/terrain-blending.png`. This remains a
  focused visual review, not the full UI gallery.
