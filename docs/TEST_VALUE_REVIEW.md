# Test-value review

Baseline: `b6272ff` on main (merged shared-harness optimizations). This review
inventoried **998 Rust tests in 142 files** and **7 Python exporter tests in two
files**. A one-time local inventory recorded each test's source location,
screening result, and any specific action. It is not checked in because it would
become stale as tests change; the lasting decisions are documented below.

## Review criteria and scope

Keep a test when it has a distinct observable contract: accepted/rejected input,
state transition, boundary condition, numeric/source-data example, deterministic
oracle, phase ordering, save/RNG behavior, UI geometry or asset decoding, or
reporting behavior. A small data-contract test can be valuable; a slow integration
test can also be valuable. Runtime and line count are not deletion criteria.

Screening examined every test's body, direct assertions, exact body duplication,
scenario-helper reuse, metadata-only cases, and potentially vacuous universal
checks. Suspect cases received direct source review, including the helper or
selector they call. Domain families were reviewed for distinct purpose. All
Rust tests and all Python tests were then executed.

The review distinguished structurally screened tests from direct-source changes
and known failures. The structural screen is not a mutation-testing proof that every
remaining assertion catches all plausible bugs, nor a new validation of every
gameplay expectation against the Civ III manual. No such blanket claim is made.
The current policy is to preserve evidence unless redundancy or false coverage
can actually be established.

No Rust test had an empty/assertion-free body, and no complete Rust test bodies
were exact duplicates. Neither fact alone establishes test quality: the concrete
problems below required inspecting what the assertions actually exercised.

## Coverage decisions

| Baseline family | Tests | Decision |
| --- | ---: | --- |
| Simulation | 770 | Retain mechanics/state/ordering/adapter coverage; revise the identified advisor, catalog, rush and scenario cases below. |
| Client | 78 | Retain decoder, asset, geometry, input and UI-data contracts; strengthen the research-ancestry fixture. |
| Map generation | 64 | Retain deterministic/oracle, topology, resource and parameter coverage; add a nonempty-size precondition to a terrain exclusion test. |
| Rules/catalog parsing | 47 | Retain source-data mappings, parser and malformed-input checks. |
| World geometry | 14 | Retain coordinate, topology, seam, boundary and direction checks. |
| Analytics/headless | 10 | Retain registration, typed provider serialization, configuration, finding lifecycle and evidence/report checks. Trace parity/collector checks are counted under simulation. |
| Playtest report/harness | 5 | Retain prerequisite gating, failure reporting and UI-script/fixture integration checks. |
| RNG | 4 | Retain native oracle/state checks. |
| Visual review reports | 3 | Retain detection and preservation of failures rather than treating unchanged pixels as success. |
| Desktop CLI | 2 | Retain malformed automation-flag and map-size checks. |
| Shared test fixture | 1 | Retain cached-map equivalence and cross-test mutation/RNG isolation. |
| Python exporters | 7 | Retain field mapping/default semantics and malformed/unsupported input checks; all pass. |

Historical Freeciv fixtures and ruleset-adapter tests remain implementation and
compatibility guards. They are not proof of Civ III parity. Existing known
assertion failures remain visible pending the corresponding implementation or
source-oracle investigation; failing status is not a reason to delete a test.

## Changes made

### Replace misleading advisor placeholders with honest catalog coverage

Seven tests in `src/sim/advisors/tests/military_advice.rs` claimed to exercise
training priorities, grave emergencies, bodyguard production, aircraft selection,
or generic defender choices, but only asserted unit metadata. They did not call
the relevant advisor behavior at all.

Remove those seven misleading test functions and regroup their **27 distinct
metadata assertions** into three appropriately named `unit_catalog` tests:

- Land candidate prerequisites and roles.
- Air/sea candidate prerequisites and combat values.
- Worker/missile capability and veterancy flags.

All distinct existing expectations were mechanically compared and preserved;
repeated assertions were consolidated. These checks now advertise their actual
data-contract scope. They do not claim that unsupported AI-policy expectations
have become behaviorally tested. The two actual military-advisor tests remain.

The hunter-role test also repeated the entire roster's cost/attack/defense
adapter comparison. Remove just that repeated loop: the catalog's full-roster
mapping test retains it, and the hunter test retains missile roles, negative
Horseman-role examples, and independent literal missile combat values.

### Strengthen actual behavior and fixtures

- **Defender selection:** compare the exact selected unit before/after adding
  training. Previously any Defense-role unit passed both checks, so a changed
  selection could go undetected. Rename the test to its actual scope rather
  than claiming unasserted cost-fallback ordering. Existing role/extra-tech/
  shield-stock checks remain. This is a selector regression contract, not a new
  claim about manual-specified AI policy.
- **Foreign-citizen rushing:** replace a lone `rush_cost(...).is_some()` check
  with matched domestic/foreign fixtures. Compare cost, execute both real rush
  commands, and check full shield stock, exact gold debit, bought-production
  flags and rejection of a second rush.
- **Research ancestry:** assert that the earlier-era critical-tech input is
  nonempty before checking membership, with the missing advance identified.
- **OpenSimplex terrain exclusions:** require the requested tile count before
  asserting that every tile excludes legacy terrain types. An empty map can no
  longer satisfy the test.
- **Technology-transfer endpoint:** accurately rename the existing 100-percent
  loss/RNG-consumption check. It does not claim coverage of arbitrary loss rates.

Other `.all(...)` checks were not automatically treated as defects. Fixed-size
arrays cannot be empty, explicit fixture counts often already establish a
nonempty case, and asserting absence after deletion intentionally allows an
empty collection.

### Split an aggregate that hid later failures

`integrated_advisor_scenarios` invoked many unrelated scenarios, but its known
ferry assertion failed before the later checks could be reached. Replace it with
eight independently reported tests for government, ferry research, civilian
upgrades, city defense, route costs, ground invasion, aircraft prerequisites,
and fighter behavior.

The growth/expansion call was already independently tested in the city-economy
module, so it is not duplicated. The two Tactical Nuke prerequisite literals
are preserved in the catalog. Existing expectations, including the intentionally
unasserted safe-city policy output, are unchanged. The fighter checks now name
unsupported interception, range limits and refuelling separately.

Seven newly independent scenario tests pass; ferry research preserves the
existing failure. This improves execution coverage and diagnosis rather than
hiding the failure or weakening an expected value.

## Verification

- Full Rust suite: **1,001 tests; 980 passed, 21 failed**, 33.29 seconds excluding
  compilation. Zero doctests. All eight split scenarios execute independently.
- The 21 failing names match the baseline after mapping the old aggregate's
  failure to its new ferry-research name. No additional failures appeared.
- Python exporters: **7 passed**, using `python3 -m unittest discover -s tools/tests -v`.
- Checked every distinct moved metadata assertion against the catalog coverage.
- Compared changed Rust files with their baseline after removing test functions,
  comments and whitespace: **non-test implementation tokens are unchanged**.
- No tests are ignored, no failure expectations are relaxed, and no golden
  fixtures, gameplay rules, source records or search budgets are changed.

The net increase from 998 to 1,001 reflects splitting useful checks, not a goal
of growing the test count. Further reductions need a concrete redundant contract
or a stronger replacement, rather than a size or speed quota.
