# Civ III terrain variation gaps

## Scope and boundary

Terrain variation has two owners:

- A map generator chooses terrain, underlying terrain, and feature placement.
- The shared map renderer chooses a sprite from the local terrain pattern and available art.

Sprite choice must not depend on whether the map came from OpenSimplex, Freeciv fractals, a saved campaign, or a review fixture. The renderer uses only the rendered tile view and neighboring tiles. The current pass is being viewed first on Freeciv-fractal maps, but its sprite rules apply to every generator. It does not change terrain placement, RNG draws, or generated map data.

## Audit and current coverage

| Terrain art | Available variations | CivCraft before this pass | Current status |
| --- | --- | --- | --- |
| Base land transitions | Multiple connected terrain combinations on transition sheets | Junction-based blending across ground types | Already varies by surrounding ground |
| Sea, coast, ocean | Dedicated water transition sheets and multiple junction cells | Separate water sheets selected from local ground junctions | Already varies by surrounding water/shore |
| Forest and jungle | Multiple atlas cells; separate grassland, plains, and tundra forest sheets, plus pine forest art | Several deterministic cells from the grassland forest atlas; no base-terrain-specific forest sheet selection | Cell variety exists; plains/tundra and pine sheet selection remain gaps |
| Marsh | Multiple atlas cells | Six deterministic cells | Existing variety retained |
| Hills | 4×4 connected-shape atlas; forest and jungle sheets | One base cell, no terrain-specific vegetation sheet | Now selects one of 16 shape cells from diagonal hilly neighbors and uses forest/jungle sheets when that vegetation dominates nearby |
| Mountains | 4×4 connected-shape atlas; forest and jungle sheets (snow-cap art intentionally unused) | One base cell; no mountain-specific variants | Now selects one of 16 shape cells and the forest/jungle sheet from nearby vegetation. Snow caps are deliberately not rendered |
| Volcanoes | 4×4 connected-shape atlas; forest and jungle sheets | Four cells from one row of the base sheet | Now selects one of 16 shape cells and the forest/jungle sheet from nearby vegetation |
| Flood plains | Multiple atlas cells | Four cells from the first row | Existing four-cell coordinate variation retained; remaining atlas cells need a source-layout check before use |
| Polar ice | Separate cap art exists in the installation | Deliberately not drawn; polar boundary remains ocean | Intentional project choice; not a terrain-variation gap |

The official mountain, hill, and volcano sheets in the inspected installation are 512×352, 512×288, and 512×352 pixels respectively. Their sprite cells are 128×88 or 128×72, giving four columns by four rows. OpenCiv3 selects a connected-shape cell from the four diagonal neighbors and chooses terrain-specific sheets for snow or nearby vegetation. See [the installed sprite comparison](CIV3_INSTALLED_SPRITE_PATH_COMPARISON.md#world-terrain-and-map-sprites) and OpenCiv3's [terrain overlay selection](../../OpenCiv3/C7/MapView.cs).

## Implemented renderer pass

`TerrainOverlayContext` carries a four-bit diagonal hilly-neighbor mask and dominant nearby vegetation into the shared Civ III renderer. Hills, mountains, and volcanoes use that mask to select the corresponding 4×4 cell. The renderer chooses forest/jungle variants from visible neighboring vegetation; if neither is dominant, it uses the base sheet. Unseen neighbors do not influence the choice.

Snow caps are deliberately not rendered. The campaign model does not store Civ III's explicit `isSnowCapped` terrain extra, and the project decision is not to add or infer this feature for presentation. The terrain renderer does not select the installed snow-cap sheet; tundra beneath a mountain does not imply a snow cap.

## Remaining work, in priority order

1. **Review actual fractal-map screenshots.** Confirm the 16-cell masks line up across mountain chains, wrap seams, and bounded map edges. Adjust the directional bit order only against the PCX atlas and rendered result.
3. **Select forest art by underlying terrain.** The model already exposes `base_terrain` for feature tiles, so forest-on-plains and forest-on-tundra art can be selected in the shared renderer. Pine forest needs its own representation or a documented presentation rule.
4. **Inspect unused atlas rows.** Floodplain, vegetation, and other multirow sheets may contain connected masks or distinct object sizes rather than interchangeable cosmetic variants. Map each row before expanding selection.
5. **Harden edition/path resolution for optional terrain sheets.** Several hill and mountain sheets live in base `Art/Terrain`, while volcano sheets are in `Conquests/Art/Terrain`. The existing conditional fallback can miss base art for installations whose root is not named `source`; verify and fix that shared lookup separately.

## Determinism and generator independence

Landform sprite cells are derived from neighboring tile terrain, not random numbers. The existing coordinate-derived selection remains in use for forests, jungle, marsh, and floodplains. These choices are stable across frames and cannot perturb seeded map generation. Both generators feed the same simulation terrain model and use this shared renderer.
