# CivCraft agent guidance

## Gameplay authority

CivCraft is a Rust implementation of Freeciv gameplay, using selected Civ III
content and artwork. The authority split is:

- **Gameplay logic** (rules, formulas, state transitions, turn processing,
  AI behavior, and rule interpretation) follows the pinned Freeciv source
  (`reference/freeciv`) and its `civ2civ3` ruleset, as copied into the
  `data/rulesets/civcraft/engine/` layer. Civ III game logic is no longer an
  authority for mechanics.
- **Civ III content and artwork** are retained only where the port explicitly
  maps them into the Freeciv model. The installed Conquests data and checked-in
  native catalogs provide identity and asset provenance; they are not gameplay
  authority.
- **Terrain, resources, and other art or asset mappings** stay with the Civ III
  sources where the port plan retains them.

Use [`docs/FREECIV_GAMEPLAY_PORT_PLAN.md`](docs/FREECIV_GAMEPLAY_PORT_PLAN.md)
and the pinned checkout under `reference/freeciv` for implementation comparisons.
When Freeciv is ambiguous, preserve the ambiguity or resolve it from the pinned
Freeciv source and ruleset; do not infer gameplay behavior from Civ III manuals,
legacy code, or community claims.

### Recorded exception: Great Leaders (Civ III)

Leader creation and use follow Civ III, as an explicit project-owner decision:
an elite land unit's kill can create a Great Leader, which can then complete a
city's current improvement or wonder. This is not a Freeciv mechanic, and the
Leader is not a `GameLoss` unit. The design is in
[`docs/superpowers/specs/2026-11-17-great-leader-design.md`](docs/superpowers/specs/2026-11-17-great-leader-design.md);
the odds are community-documented for Civ III patch 1.29f and live in
`data/rulesets/civcraft/content/settlements.toml`. Keep other mechanics on the
Freeciv authority above.

## Comparison source checkout

The Freeciv source submodule at `reference/freeciv` tracks the public upstream
`main` branch. It is the gameplay authority for mechanics, but the pinned revision
recorded in the port plan governs what CivCraft ports. Consult it for any
gameplay logic, formula, or state transition. Content records (units, buildings,
civilizations) still come from Civ III, and Freeciv's content does not replace
them. For example, the Civ2Civ3 unit rules are in
`reference/freeciv/data/civ2civ3/units.ruleset`; the Civ III unit catalog
overrides its unit identities and values.

## Choosing the right test path

Use the narrowest test path that exercises the behavior being changed:

- Simulation rules, turn processing, AI behavior, or deterministic gameplay
  scenarios: use the headless simulator. Prefer a registered scenario for a
  reusable regression and `--load-campaign` for an elaborate saved starting
  state. See `docs/HEADLESS_ANALYTICS.md`.
- Headless executable arguments, exit status, and report contracts: add an
  `assert_cmd` integration test under `tests/`. Do not duplicate engine-rule
  assertions at the CLI layer.
- Rendering, layout, screenshots, or visual regressions: use `--ui-review`.
  Compare against an explicit baseline for regression work; never update a
  baseline implicitly. Run it only when `CIVCRAFT_SOURCE_DIR` is set and points
  to a complete Civ III installation. Otherwise skip visual testing, report the
  missing prerequisite, and do not describe the skipped review as passing. See
  `docs/UI_REVIEW.md`.

Run the regular Rust suite locally with Cargo:

```sh
cargo test --all-targets
cargo test --doc
```

Use focused test filters while iterating. Run the broader commands before
hand-off when the change warrants it. Keep the separate `cargo test --doc` step
because `--all-targets` does not run documentation tests. Nextest is for CI:
CI runs `cargo nextest run --profile ci --all-targets` using
`.config/nextest.toml`, followed by `cargo test --doc`.

For analytics review, start with `report.json` findings and the named summaries
under `analytics.metrics` (`government.nations`, `government.recent_changes`,
`diplomacy.relationships`, and `diplomacy.recent_activity`). Follow their evidence
references into the sidecars instead of dumping full JSONL streams. Default
`--report-detail lean` output includes important adapter changes, compact events,
and full snapshots at checkpoints and finding boundaries. Read
`evidence.changes` for the change stream. Use `--report-detail verbose` for full
per-boundary snapshots and every selected event. Analyzers receive all collected
events and full snapshots in either mode.

Start with lean mode for general reviews. Choose verbose mode when the question
requires exact per-turn food, research progress, treasury, movement, work
countdowns, or every government decision; when a missing intermediate snapshot
blocks a diagnosis; or when validating an adapter or compaction rule. Rerun the
same seed/world settings or saved campaign with a focused turn range and relevant
providers/domains. Check coverage and retention metadata before treating an
absent record as an absent action: verbose mode still honors trace filters and
limits, and cannot expose uninstrumented actions. Enable `--profile-turns true`
separately when investigating execution cost. See `docs/HEADLESS_ANALYTICS.md`
for coverage and retention semantics.

### Headless scenarios and saved campaigns

List and run registered scenarios without creating a graphics context:

```sh
cargo run --offline --release --bin headless_sim -- --list-scenarios
cargo run --offline --release --bin headless_sim -- \
  --scenario settlement-ferry --turns 1 --report /tmp/civcraft-ferry.json
cargo run --offline --release --bin headless_sim -- \
  --load-campaign /path/to/campaign.save --turns 10 \
  --report /tmp/civcraft-campaign.json
```

Require a zero exit status and inspect `summary.scenario_checks_passed`,
`summary.scenario_checks_total`, and every entry under `run.scenario.checks`.
Failed scenario checks make the command fail after it publishes the report.
The lifecycle is prepare, attach tracing/analyzers, execute scenario actions,
capture the action boundary, simulate turns, then finish checks. Put reusable
state mutation in simulation-level scenario helpers; compose those helpers in
scenario definitions instead of reaching into desktop playtest code.

For focused screenshot galleries and visual review, see `docs/UI_REVIEW.md` and
the `--ui-review` instructions in `README.md`. For a visual regression
comparison, supply both the current output and an explicit baseline:

```sh
./target/release/civcraft --seed 42 \
  --ui-review /tmp/civcraft-ui-current \
  --ui-baseline /path/to/civcraft-ui-baseline \
  --ui-min-similarity 0.99 --window-size 1280x720
```

Treat a missing baseline, dimension mismatch, similarity below the threshold,
or any failed report check as a failure. Review generated `*-diff.png` files
and the `baseline_similarity` values in `report.json`. Do not copy current
captures over the baseline unless the user explicitly approves a baseline
change.

For this checkout, the Civ III art source is available at
`$HOME/Documents/projects/CivCraft/source`. Set it before launching any game
window or screenshot/visual-review run:

```sh
export CIVCRAFT_SOURCE_DIR="$HOME/Documents/projects/CivCraft/source"
```

Before launching `--ui-review`, verify that the configured installation has
the required terrain assets:

```sh
test -n "${CIVCRAFT_SOURCE_DIR:-}" \
  && test -d "$CIVCRAFT_SOURCE_DIR/Conquests/Art/Terrain"
```

If that preflight fails, do not run `--ui-review` or a baseline comparison in
that environment. Continue with applicable headless and Rust tests and state
that visual testing was skipped because the Civ III installation was not
available. Do not substitute fallback, generated, or partial assets for an
authoritative visual baseline.

## Checking map-generation goldens

Campaigns use Civ III's horizontal map topology and 36×48 tile preset. All
campaign maps wrap horizontally and have bounded polar edges. Generate maps
with the desired dimensions from the start; changing `sim.map_wrap` after
generation leaves arrays inconsistent.

`tests/fixtures/freeciv_mapgen_vectors.tsv` contains raw Freeciv 3.3.90.14-dev
(eb8c703) terrain, altitude, extras and starting locations for historical
comparison only. It is not the Civ III target. The parity test checks the
unmodified legacy generator against this file.
`tests/fixtures/civcraft_mapgen_vectors.tsv` contains checked CivCraft output
using the Civ III resource catalog. Resource placement changes extras and start
site scoring, so keep these goldens separate from the native reference.

The historical generator is no longer distributed with this repository. If a
fixture must be replaced, reproduce it independently with the exact recorded
Freeciv revision and reference Civ2Civ3 data. The reference server binds
localhost even without clients, so sandboxed runs may need desktop/network
approval. Unit goldens do not validate rendering.
