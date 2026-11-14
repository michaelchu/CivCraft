# CivCraft agent guidance

## Civilization III gameplay manual

Use [`docs/civ3_manual.pdf`](docs/civ3_manual.pdf) at the project root as the primary
reference for Civilization III gameplay, terrain, resources, units, buildings,
and other rules. Record manual-derived expectations in
[`docs/CIV3_MANUAL_RULES.md`](docs/CIV3_MANUAL_RULES.md), and use
[`docs/GAMEPLAY_GAP_AUDIT.md`](docs/GAMEPLAY_GAP_AUDIT.md) for future comparisons with
the implementation. The catalog has begun recording manual-sourced terrain
rules; a gameplay gap audit has not been performed.

When the manual does not specify a detail needed to establish an expected
behavior, research Civ III-specific online sources. Prefer original game
materials and official publications where available, then well-sourced
specialist references, including community resources such as CivFanatics and
Civ Wiki. These community sources are useful leads for undocumented or
reverse-engineered mechanics and formulas; treat them as secondary evidence,
not as authoritative game documentation. Follow their citations to original
materials where possible and check which edition, expansion, or patch the
claim covers.

Do this research while implementing any mechanic whose expected behavior or
formula is not established by the manual or other primary evidence; do not
silently fill gaps from legacy code or assumptions. Record the source title,
author or publisher when available, URL, edition/expansion/patch scope,
relevant page or section, date checked, and the specific claim it supports in
`docs/CIV3_MANUAL_RULES.md`. Label supplementary claims separately from
manual-derived claims, note when a formula is reverse-engineered or inferred,
preserve conflicting evidence, and leave unresolved details explicit rather
than guessing. Freeciv behavior, existing code, tests, and previous parity
claims are not Civ III evidence.

Use the catalog's [source-record format](docs/CIV3_MANUAL_RULES.md#source-records),
[technology-tree source leads](docs/CIV3_MANUAL_RULES.md#technology-tree-source-leads),
and [general gameplay-mechanics index](docs/CIV3_MANUAL_RULES.md#general-gameplay-mechanics-index)
as starting points, not blanket endorsements. The Vi.ki pages are secondary
summaries that cite a mixture of manuals, Prima's guide, previews, and archived
official material. Follow those citations and prefer the specific underlying
source for each claim; record edition differences or disagreements.

## Comparison source checkout

The Freeciv source submodule at `reference/freeciv` tracks the public upstream
`main` branch and is implementation context only, not a gameplay authority. Use it
only when a task explicitly requires examining that implementation. Do not
copy its behavior into the manual-derived expected rules. For example, the
Civ2Civ3 unit rules are in `reference/freeciv/data/civ2civ3/units.ruleset`.

## Choosing the right test path

Use the narrowest test path that exercises the behavior being changed:

- Simulation rules, turn processing, AI behavior, or deterministic gameplay
  scenarios: use the headless simulator. Prefer a registered scenario for a
  reusable regression and `--load-campaign` for an elaborate saved starting
  state. See `docs/HEADLESS_ANALYTICS.md`.
- Headless executable arguments, exit status, and report contracts: add an
  `assert_cmd` integration test under `tests/`. Do not duplicate engine-rule
  assertions at the CLI layer.
- Real mouse/keyboard routing, widget actions, or behavior that requires the
  desktop client: use the built-in `--self-test` gameplay scenarios below.
- Rendering, layout, screenshots, or visual regressions: use `--ui-review`.
  Compare against an explicit baseline for regression work; never update a
  baseline implicitly. Run it only when `CIVCRAFT_SOURCE_DIR` is set and points
  to a complete Civ III installation. Otherwise skip visual testing, report the
  missing prerequisite, and do not describe the skipped review as passing. See
  `docs/UI_REVIEW.md`.

Run the regular Rust suite with the repository's nextest profile:

```sh
cargo nextest run --profile ci --all-targets
cargo test --doc
```

Use focused test filters while iterating. Run the broader commands before
hand-off when the change warrants it. Nextest does not run documentation tests,
so keep the separate `cargo test --doc` step. CI uses the same commands and the
configuration in `.config/nextest.toml`; see `docs/TEST_PERFORMANCE.md`.

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

## Running a requested desktop playtest

When asked specifically for a desktop or UI interaction playtest, run the
built-in desktop gameplay scenarios. They launch the real game window,
exercise gameplay and UI behavior, write a JSON report, and exit
automatically. Rust unit tests or a headless scenario alone do not constitute
this desktop playtest.

Run from this directory (`app/`):

```sh
cargo build --offline --release
playtest_dir=$(mktemp -d /tmp/civcraft-playtest-XXXXXX)
./target/release/civcraft --self-test "$playtest_dir/report.json" --seed 42
```

Use a fresh report path for each run so an old report cannot be mistaken for
the current result. Seed 42 is the default reproducible scenario seed; honor
another seed if the user requests one.

### macOS desktop access

The game needs access to the macOS desktop to create its graphics context.
In a sandboxed agent environment, launch the executable with the tool's
`sandbox_permissions: "require_escalated"` option and explain that desktop
access is needed for the playtest window. The release build can run inside
the sandbox.

A sandboxed launch was observed to fail before gameplay with:

```text
vendor/miniquad/src/native/macos.rs
assertion failed: !glpixelformat_obj.is_null()
```

If this occurs, retry the executable with desktop access. Do not treat this
graphics initialization failure as a failed gameplay scenario. If desktop
access is unavailable, report that the desktop playtest could not run.

### Verify and report the result

Wait for the executable to exit, then inspect the report. Require both a zero
exit status and every entry in `steps` having `passed: true`. Report the total
passed checks, failed checks with their `step` and `detail`, the seed, and a
link to the report. Do not assume a fixed number of checks; the suite grows.

For example, substituting the actual report path:

```sh
python3 - <<'PY'
import json
from pathlib import Path

path = Path('/tmp/civcraft-playtest-XXXXXX/report.json')
steps = json.loads(path.read_text())['steps']
failures = [step for step in steps if not step['passed']]
print(f'{len(steps) - len(failures)}/{len(steps)} checks passed')
for step in failures:
    print(f"FAIL {step['step']}: {step['detail']}")
if not steps or failures:
    raise SystemExit(1)
PY
```

### Clean up completed runs

After verifying each run, keep the latest successful report and all failed
reports. Remove older successful reports generated by this workflow. Keep the
retained report at its original path so links in the chat remain usable.

Only delete a report when its run is known to have exited successfully and its
nonempty `steps` list contains exclusively `passed: true`. Preserve incomplete,
unreadable, or uncertain results for investigation. A failed new run must not
remove the previous successful report.

Limit cleanup to this workflow's `/tmp/civcraft-playtest-*/report.json` files
and the legacy `/tmp/civcraft-playtest*.json` reports. Inspect candidates before
deleting; do not recursively delete directories or use a wildcard removal
command. Remove a run directory only if it is empty after removing its report.
Honor any user request to retain particular runs. Mention the retained report
in the final response.

For focused screenshot galleries and visual review, see `docs/UI_REVIEW.md` and
the `--ui-review` instructions in `README.md`. Run that mode separately from
`--self-test`. For a visual regression comparison, supply both the current
output and an explicit baseline:

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
