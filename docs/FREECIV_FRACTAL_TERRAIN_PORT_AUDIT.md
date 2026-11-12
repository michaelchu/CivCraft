# Freeciv fractal terrain port audit

## Scope and verdict

Compared CivCraft's `FreecivFractalMapGenerator` and `freeciv_fractal_height_field` in `app/src/mapgen.rs` with Freeciv's `make_pseudofractal1_hmap` / `gen5rec` in `freeciv/server/generator/height_map.c` and its orchestration in `freeciv/server/generator/mapgen.c`.

**Verdict:** the recursive midpoint-displacement idea and much of the subdivision order are ported. This is an adaptation of Freeciv's FRACTAL height-field algorithm, not a full or seed-compatible port of Freeciv map generation. It is now CivCraft's default landmass-field source, but should not be described as complete Freeciv terrain generation or as reproducing Freeciv maps.

The CLI selects this generator by default and accepts `--map-generator fractal`. `--map-generator opensimplex` selects the alternate OpenSimplex implementation.

This is a source comparison. No runtime comparison against Freeciv and no tests were run for this audit.

## What matches

- Both initialize a coarse grid, recursively set side and center midpoints from corner averages plus bounded random jitter, recurse in the same four-quadrant order, add final per-cell fuzz, then scale the resulting height field.
- Both use a starting displacement proportional to map width plus height, reduce displacement by roughly two thirds at each recursion, and avoid overwriting already-set shared points.
- CivCraft correctly models the intended default X-wrapped, Y-bounded grid shape: coarse points include both Y boundaries, while X coordinates wrap at the seam.
- CivCraft keeps this height-field algorithm separate from its subsequent Civ III-oriented terrain, rivers, resources, and start-position passes. Those downstream passes are not Freeciv's terrain-generation pipeline.

## Gaps from Freeciv

1. **Subdivision count is different.** Freeciv uses `xdiv = ydiv = 5 + extra_div`, where the default/all start-position modes use `extra_div = 1`; other start-position modes add player count. CivCraft picks 4, 6, or 8 divisions from its Archipelago/Continents/Pangaea setting. This is a CivCraft feature-size mapping, not Freeciv's start-position-dependent behavior.
2. **Random stream and draw sequence differ.** Freeciv uses `fc_rand` from its map-generation RNG and shares the map seed stream with later passes (`height_map_to_map` reseeds from the map seed). CivCraft uses its own `MapRng`, per-field seed offsets (`0xfeed`, `0xdad`, and separate retry seeds), and ranks fields before terrain classification. Even where formulas look similar, output and downstream draws cannot match Freeciv.
3. **Initial edge/pole treatment differs.** Freeciv subtracts `avoidedge` at actual topological singularities, applies a random pole adjustment based on `flatpoles`, and has special midpoint handling near singularities and poles. CivCraft applies edge avoidance to a row band (`y <= 2` or `y >= height - 3`) only for the water field; its midpoint setter uses generic unset-point behavior. CivCraft's height-field variants for relief and biome omit this edge treatment.
4. **Pole lowering is deliberately non-equivalent.** The Civ2Civ3 ruleset in this checkout sets `flatpoles` to 0. CivCraft instead lowers the water field with a hard-coded 100% factor taper and later forces the outer row to ocean. This follows CivCraft's polar-ocean policy, not the checked Freeciv ruleset.
5. **Normalization and field usage differ.** Freeciv calls `adjust_int_map(height_map, 0, hmap_max_level)` and uses the resulting height map as altitude for its own land/terrain pipeline. CivCraft min-max normalizes each fractal field to 0..255, then percentile-ranks the water field and sends it through CivCraft's shared terrain pass. It also generates independent fractal relief and biome fields that Freeciv's FRACTAL height-map routine does not create.
6. **Topology coverage is narrower.** The implementation assumes CivCraft's map-wrap/index abstraction and its bounded polar rows. It does not reproduce Freeciv's general native-coordinate behavior for independently wrapped or unwrapped X/Y axes, topology singularities, and their special initialization rules.
7. **Freeciv post-processing is absent by design.** Freeciv follows height generation with pole normalization, land creation, tiny-island removal, water-depth smoothing, continent assignment, and lake regeneration in its own generator. CivCraft uses different terrain classification, water finishing, river, feature, and start allocation logic.

## Follow-up needed for a closer parity claim

- Choose and state a specific Freeciv ruleset, map topology, map size, start-position mode, and seed/RNG contract as the comparison target.
- Port the Freeciv division calculation, native-coordinate/singularity handling, flat-pole semantics, and height-map normalization under that target.
- Add a deterministic field-level comparison harness that captures Freeciv and CivCraft height maps at the same stage. Until that exists, formula similarity is not evidence of output parity.
- Decide whether the intended goal is just a Freeciv-inspired landmass option for Civ III maps or actual Freeciv FRACTAL-generator parity. If it is the former, keep the adaptations explicit and avoid calling the full pipeline a port.

## Source pointers

- CivCraft: `app/src/mapgen.rs`, especially `Landform::fractal_divisions`, `Generator::generate_with_landmass`, `freeciv_fractal_height_field`, `freeciv_fractal_subdivide`, and `lower_freeciv_poles`.
- Freeciv fractal algorithm: `freeciv/server/generator/height_map.c`, `make_pseudofractal1_hmap` and `gen5rec`.
- Freeciv generator orchestration: `freeciv/server/generator/mapgen.c`, including `height_map_to_map` and the FRACTAL branch.
- Checked ruleset value: `freeciv/data/civ2civ3/game.ruleset` (`generator = FRACTAL`, `flatpoles = 0`).
