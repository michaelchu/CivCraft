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
  `data/rulesets/civcraft_civ2civ3/`. `classic` and other rulesets must not be
  used to fill gaps. The sibling `default/` and `nation/` files are dependencies
  in the selected ruleset's include closure, not alternate gameplay baselines.
- Civ III: Conquests source records remain authoritative for campaign unit,
  building, and technology identities and values. C2C3 records may supply a
  Freeciv mechanic profile only through an explicit adapter; that does not make
  the C2C3 record the Civ III source of truth.
- Gameplay state and transitions stay in `src/sim/`; rules and catalogs stay in
  `src/rules/` and `data/rulesets/`; rendering and asset lookup stay behind the
  client/view boundary. Keep shared requirement/effect/action evaluation as
  the single path for calculations.

## Change the CivCraft ruleset

1. Edit the corresponding file under
   `data/rulesets/civcraft_civ2civ3/` and record the reason and intended
   Freeciv behavior in the change description. Keep copied C2C3 records and
   CivCraft-specific overrides distinguishable.
2. Keep inherited engine defaults that affect gameplay explicit in
   `game.ruleset` when the campaign relies on them. Do not silently fall back
   to values from another Freeciv ruleset.
3. If a rule name, requirement, effect, action, or content universal changes,
   update the owning adapter in `src/rules/` or `src/sim/` and its focused
   source-backed checks. Avoid adding a second subsystem-specific calculation.
4. If copied upstream files change, update
   `data/rulesets/civcraft_civ2civ3/UPSTREAM_MANIFEST.tsv` with the pinned
   source and destination SHA-256 values. Keep `identical` and `modified`
   statuses accurate; the modified destination is an intentional CivCraft
   override, not a new upstream source. Compute both values with
   `shasum -a 256 <source-file> <destination-file>` from their respective
   checkout roots and compare the output with the manifest row.
5. Check that `RulesetData` still loads the selected baseline and its required
   include closure. The `packaged_baseline_loads_only_selected_civcraft_ruleset_files`
   check in `src/rules/freeciv.rs` records the current selected-file and record
   counts.

When advancing Freeciv, use a clean checkout or detached worktree for the new
full commit. Review changes to `data/civ2civ3/`, inherited `data/default/`
hooks, the included nation files, and the relevant `common/` and `server/`
implementations before copying anything. Preserve the current CivCraft
overrides; copy only the selected C2C3 files and required include dependencies.
Update the revision in this guide, the port plan, the baseline README and
manifest, plus any fixture metadata affected by the change. If generated engine
identifiers or ruleset enums change, update their source-derived tables in
`src/rules/freeciv/identifiers.rs` and the adapters that consume them.

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
| Units and unit art | `data/rulesets/civ3_conquests/units.tsv` has 92 campaign identities, source values, unique replacements, requirements, and `art_folder` / `art_root`. `civilization_starts.tsv` and `experience_levels.tsv` hold associated start and rank inputs. | `UnitKind::from_conquests_id` in `src/sim/model/units.rs` resolves stable identities. Add or correct a source row with `tools/export_civ3_conquests_units.py`, then check the identity, art path, unique-unit, and start-profile consumers. |
| Unit mechanics profiles | `data/rulesets/civcraft_civ2civ3/unit_economics.tsv` has 92 explicit C2C3 profiles: 33 direct matches and 59 role aliases for unit economics. | `src/sim/action_rules.rs` also maps C2C3 unit type records and flags; its explicit Galley-to-Trireme alias and no-type/default ledger are separate from the economics table. For a new or changed unit, add a sourced direct mapping, a justified role alias, or an explicit unsupported entry. Do not copy C2C3 combat, movement, or art values into the Civ III catalog. |
| Buildings | `data/rulesets/civ3_conquests/buildings.tsv` retains all 83 BIQ records. | `building_catalog_bindings.tsv` maps 47 records to stable legacy `Building` IDs; 36 have explicit unresolved reasons. Add a stable binding only when the identity is deliberate and unique. Keep unresolved reasons until then; never assign by BIQ or enum row order. |
| Technologies | `data/rulesets/civ3_conquests/technologies.tsv` retains all 83 BIQ advances and prerequisites. | `src/sim/content_adapter.rs` maps Freeciv technology names to the Civ III tree. Current explicit name bridges include Superconductors/Superconductor, Amphibious Warfare/Amphibious War, Conscription/Nationalism, Environmentalism/Ecology, Laser/The Laser, Mobile Warfare/Motorized Transportation, and Nuclear Fission/Fission. C2C3 Radio maps to Civ III Flight; Freeciv-only Plastics has no Civ III advance. Maintain the adapter and research consumers when either source name or identity changes. |
| Civilizations | `civilizations.tsv` preserves 31 playable Conquests identities; `civilization_starts.tsv` preserves their campaign start profiles. | `src/rules/civilizations.rs` and `src/sim/civilization_start_rules.rs` load them. The `city_style` field is a CivCraft renderer mapping, not a BIQ field. Keep roster IDs and art style bindings explicit. |
| Terrain and resources | The catalog uses Civ III terrain/resource identities and yields, but these are not currently standalone TSV catalogs under `civ3_conquests/`. | Terrain name adaptation is in `src/mapgen/terrain.rs`; resource identities, habitats, and yields are in `src/world/terrain.rs`; Freeciv placement adapted to those resources is in `src/mapgen/placement.rs`. Record this as code-owned mapping when changing it; do not claim it is editable through the C2C3 ruleset. |
| Other art | Unit art roots come from `units.tsv`; civilization city styles come from `civilizations.tsv`. Other terrain, improvement, road, river, and UI sprite selection is resolved by the existing client tileset/theme loaders. | Update the owning mapping in `src/client/tileset.rs` and its asset/theme coverage checks. Keep presentation mapping out of simulation rules. |

`src/sim/content_adapter.rs::civ3_source_content_binds_to_stable_ids_and_reports_catalog_gaps`
is the current identity-count contract: 92/92 units, 83/83 technologies,
31/31 civilizations, and 47/83 buildings mapped to legacy wrappers, with the
remaining 36 buildings carrying explicit reasons. The `Amphibious War` BIQ
identity binds to the stable `Amphibious Warfare` technology ID. Keep serialized
unit, building, and technology IDs stable; if persistent representation must
change, make an explicit save-version and migration decision.

## Extend parity fixtures

Use the comparison ledger in the port plan to select the owning subsystem and
native source function. Each Freeciv fixture must record its own exact source
revision, ruleset, relevant function or setup, and any server/library/compiler
hashes needed to reproduce it. Do not assume every existing fixture uses the
current pin: `freeciv_mapgen_vectors.tsv` intentionally records the earlier
3.3.90.14-dev `eb8c703` oracle, while several later fixtures record
`9ff184b6eb128ca636c7ad6269e27aa959600006`.

Existing oracle generators include:

- `tools/generate_freeciv_combat_oracles.py` for the extracted native combat
  probability function;
- `tools/generate_border_oracles.py` for native server border output plus
  separately labeled Civ III topology cases;
- `tools/generate_mapgen_oracles.py` for native server terrain, extras, and
  starts; and
- the checked-in fixture-specific oracle scripts for random-number,
  counter-research, and normal-tax vectors.

The combat and border generators use the current `9ff184b6` source pin. The
checked-in native mapgen vector predates that pin, so regenerate it only with
the matching `eb8c703` server and data. Do not pair a server binary with data
from a different Freeciv revision. From `app/`, the commands are:

```sh
python3 tools/generate_freeciv_combat_oracles.py --reference ../freeciv
python3 tools/generate_border_oracles.py --server <freeciv-9ff184b6-server> \
  --data ../freeciv/data --source-root ../freeciv \
  --work /tmp/civcraft-border-oracles \
  --output tests/fixtures/freeciv_border_vectors.tsv
python3 tools/generate_mapgen_oracles.py \
  --server <freeciv-eb8c703-server> --data <freeciv-eb8c703-data> \
  --work /tmp/civcraft-mapgen-oracles \
  --output tests/fixtures/freeciv_mapgen_vectors.tsv
```

Use each generator's `--help` and embedded usage before regenerating. Run the
native reference with the intended pinned source and `civ2civ3` data; do not
make an oracle output match CivCraft by modifying the Freeciv fixture. Add the
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
saves use header `CIVCRAFT\0\x22`. Older save headers are rejected and are not
migrated. A future persistent-state change must explicitly choose a save-version
policy and cover round-trip, invalid-input, and compatibility behavior before
the corresponding plan item is complete.

P14 is complete when a maintainer can identify the authority and owner for a
ruleset change, content mapping, upstream revision, and parity vector; every
unresolved mapping remains explicit; and the port plan records the current
compatibility outcome and intentional differences.
