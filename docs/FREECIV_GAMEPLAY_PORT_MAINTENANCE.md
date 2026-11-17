# Freeciv gameplay port maintenance

This guide is the maintainer workflow for the single-player Freeciv gameplay
port. It explains where to change the CivCraft ruleset, how to preserve Civ III
campaign content, how to update the pinned Freeciv source, and how to extend
parity fixtures. The [port plan](FREECIV_GAMEPLAY_PORT_PLAN.md) remains the
scope, status, and subsystem-difference ledger.

## Authority and boundaries

- Freeciv engine behavior and the initial gameplay baseline come only from the
  pinned revision and its `civ2civ3` ruleset. The current pin is
  `9ff184b6eb128ca636c7ad6269e27aa959600006`.
- Editable gameplay data belongs in
  `data/rulesets/civcraft/engine/`. `classic` and other rulesets must not be
  used to fill gaps. The sibling `default/` and `nation/` files are dependencies
  in the selected ruleset's include closure, not alternate gameplay baselines.
- Civ III: Conquests source records provide retained campaign identities,
  values, and provenance only. Freeciv source and the selected C2C3 ruleset are
  authoritative for gameplay mechanics and behavior.
- Gameplay state and transitions stay in `src/sim/`; rules and catalogs stay in
  `src/rules/` and `data/rulesets/`; rendering and asset lookup stay behind the
  client/view boundary. Keep shared requirement/effect/action evaluation as
  the single path for calculations.

## Change the CivCraft ruleset

1. Edit the corresponding file under `data/rulesets/civcraft/engine/`.
   `actions.ruleset` owns actions and enablers, `effects.ruleset` owns
   effects and their requirements, and `terrain.ruleset` owns
   `[parameters].move_fragments`. Record the reason and intended Freeciv
   behavior in the change description.
2. Keep inherited engine defaults that affect gameplay explicit in
   `game.ruleset` when the campaign relies on them. Do not silently fall back
   to values from another Freeciv ruleset.
3. If a rule name, requirement, effect, action, or content universal changes,
   update the owning adapter in `src/rules/` or `src/sim/` and its focused
   source-backed checks. Avoid adding a second subsystem-specific calculation.
4. If copied upstream files change, update
   `data/rulesets/civcraft/engine/UPSTREAM_MANIFEST.tsv` with the pinned
   source and destination SHA-256 values. Keep `identical` and `modified`
   statuses accurate; the modified destination is an intentional CivCraft
   override, not a new upstream source. Compute both values with
   `shasum -a 256 <source-file> <destination-file>` from their respective
   checkout roots and compare the output with the manifest row.
5. Check that `RulesetData` still loads the selected baseline and its required
   include closure. The `packaged_baseline_loads_only_selected_civcraft_ruleset_files`
   check in `src/rules/engine_rules.rs` records the current selected-file and record
   counts.

When advancing Freeciv, use a clean checkout or detached worktree for the new
full commit. Review changes to `data/civ2civ3/`, inherited `data/default/`
hooks, the included nation files, and the relevant `common/` and `server/`
implementations before copying anything. Preserve the current CivCraft
overrides; copy only the selected C2C3 files and required include dependencies.
Update the revision in this guide, the port plan, the baseline README and
manifest, plus any fixture metadata affected by the change. If generated engine
identifiers or ruleset enums change, update their source-derived tables in
`src/rules/engine_rules/identifiers.rs` and the adapters that consume them.

For example, from `app/`, check the source revision and review the selected
ruleset diff before importing it:

```sh
git -C ../freeciv rev-parse HEAD
git -C ../freeciv diff <old-revision>..<new-revision> -- \
  data/civ2civ3 data/default/default.lua data/default/nationlist.ruleset
cargo test --offline packaged_baseline_loads_only_selected_civcraft_ruleset_files
```

## Update Civ III content mappings

The source catalog and the Freeciv-mechanics bridge are separate layers. Keep
source BIQ indices, IDs, and hashes intact; do not infer a mapping from row
order or a similar display name.

| Content | Source data | Runtime bridge and maintenance rule |
|---|---|---|
| Units and unit art | `data/rulesets/civcraft/content/units.toml` has 92 campaign identities, source values, unique replacements, requirements, and `art_folder` / `art_root`. `civilizations.toml` and `experience_levels.toml` hold associated start and rank inputs. | `UnitKind::from_conquests_id` in `src/sim/model/units.rs` resolves stable identities. When correcting a native record, preserve its stable ID, then check the identity, art path, unique-unit, and start-profile consumers. |
| Unit mechanics profiles | Native unit statistics and capabilities live in `data/rulesets/civcraft/content/units.toml`; the former unit-economics crosswalk has been removed. | `src/sim/action_rules.rs` maps native unit capabilities into the shared Freeciv requirement evaluator. For a new or changed unit, update the native record and its focused consumers. Do not copy C2C3 combat, movement, or art values into the Civ III catalog. |
| Buildings | `data/rulesets/civcraft/content/buildings.toml` contains 91 active native rules. | Native building rules are authoritative; `src/rules/compatibility.rs` preserves serialized IDs and the existing obsolescence policy. The unused source records have been removed. Never infer save identities from author record order. |
| Technologies | `src/rules/technologies.rs` is the sole source for all 83 researchable native advances, stable runtime indices, display aliases, prerequisites, eras, and era gates. | `src/rules/research.rs` builds the runtime catalog from the native records; `src/sim/content_adapter.rs` maps Freeciv compatibility names into it. Current explicit bridges include Superconductors/Superconductor, Amphibious Warfare/Amphibious War, Conscription/Nationalism, Environmentalism/Ecology, Laser/The Laser, Mobile Warfare/Motorized Transportation, and Nuclear Fission/Fission. C2C3 Radio maps to Civ III Flight; Freeciv-only Plastics has no Civ III advance. `engine/techs.ruleset` remains a generic compatibility/schema input and must not be edited to change the Civ III research graph. |
| Civilizations | `data/rulesets/civcraft/content/civilizations.toml` preserves 31 playable Conquests identities and their campaign start profiles. | `src/rules/civilizations.rs` and `src/sim/civilization_start_rules.rs` load them. The `city_style` field is a CivCraft renderer mapping, not a BIQ field. Keep roster IDs and art style bindings explicit. |
| Terrain and resources | `data/rulesets/civcraft/content/terrain.toml` and `economy.toml` own the migrated native terrain/resource inputs. | Terrain name adaptation is in `src/mapgen/terrain.rs`; resource identities, habitats, and yields are in `src/world/terrain.rs`; Freeciv placement adapted to those resources is in `src/mapgen/placement.rs`. Record this as code-owned mapping when changing it; do not claim it is editable through the C2C3 ruleset. |
| Other art | Unit art roots come from `civcraft/content/units.toml`; civilization city styles come from `civcraft/content/civilizations.toml`. Other terrain, improvement, road, river, and UI sprite selection is resolved by the existing client tileset/theme loaders. | Update the owning mapping in `src/client/tileset.rs` and its asset/theme coverage checks. Keep presentation mapping out of simulation rules. |

`src/sim/content_adapter.rs` maps engine technology names to compiled native
technology IDs. Runtime source-record catalogs and BIQ metadata have been removed.
The native catalogs and compatibility table preserve existing identities and
obsolescence behavior. Keep serialized unit, building, and technology IDs stable;
if persistent representation must change, make an explicit save-version and
migration decision.

## Extend parity fixtures

Use the comparison ledger in the port plan to select the owning subsystem and
native source function. Each Freeciv fixture must record its own exact source
revision, ruleset, relevant function or setup, and any server/library/compiler
hashes needed to reproduce it. Do not assume every existing fixture uses the
current pin: `freeciv_mapgen_vectors.tsv` intentionally records the earlier
3.3.90.14-dev `eb8c703` oracle, while several later fixtures record
`9ff184b6eb128ca636c7ad6269e27aa959600006`.

Oracle generators are no longer distributed in this repository. The combat
and border fixtures use the `9ff184b6` source pin. The checked-in native mapgen
vector predates that pin and records `eb8c703`. Reconstruct any replacement
independently from the exact recorded source, server, data, and setup; never
pair a server binary with data from a different revision or alter an oracle to
match CivCraft. Add the
new vector to the owning subsystem's test module and label source-native results
separately from CivCraft adaptations. For a mechanics change, run its focused
check and then `cargo test --offline --all-targets -- --test-threads=4` plus
`cargo build --offline --release`; keep desktop/UI review separate.

Keep `tests/fixtures/freeciv_mapgen_vectors.tsv` separate from
`tests/fixtures/civcraft_mapgen_vectors.tsv`: the former is historical native
Freeciv output, while the latter is a CivCraft self-regression baseline with
Civ III resource placement and start scoring. For intentional topology,
content, or RNG differences, retain the Freeciv vector and record the adaptation
beside the CivCraft result rather than overwriting either expectation.
Regenerate the CivCraft self-regression vector with
`cargo test --offline dump_civcraft_mapgen_vectors -- --ignored --nocapture`.

At minimum, preserve fixture provenance and report failures by case and detail.

## Recorded differences and compatibility

The subsystem comparison ledger in `FREECIV_GAMEPLAY_PORT_PLAN.md` is the
source of truth for intentional differences and deferred work. In particular,
it records the Civ III unit/building/technology catalog, Civ III terrain and
resource adaptation, X-wrap and bounded-Y map topology, omitted Freeciv-only
Storm units, road/Harbor-based trade-route creation, and espionage deferred by
the user. Update the owning ledger row when one of these decisions changes.

The gameplay simulation remains authoritative behind CivCraft's existing
command/client-view boundary; rendering consumes simulation state. Current
saves use header `CIVCRAFT\0\x27`. The loader reads `0x26` campaigns and
writes the new format on the next save. Round-trip checks cover supported
map sizes through 512×512. Future persistent-state changes should specify the
save version and cover round-trip, invalid-input, and compatibility behavior.

P14 is complete when a maintainer can identify the authority and owner for a
ruleset change, content mapping, upstream revision, and parity vector; every
unresolved mapping remains explicit; and the port plan records the current
compatibility outcome and intentional differences.
