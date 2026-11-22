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
OpenSimplex path already assigned feature bases independently. This is not lost
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
