# Civilization III gameplay gap audit

Status: **Not started.** This is a worksheet for a future audit against
[`CIV3_MANUAL_RULES.md`](CIV3_MANUAL_RULES.md). No rules have been compared
with implementation and no gaps have been assessed in creating this document.

## Audit principles

1. Start from an entry in `CIV3_MANUAL_RULES.md`; do not treat existing code,
   tests, Freeciv behavior, or previous parity claims as the expected result.
2. Prefer the manual. When it is silent on a detail needed for an expected
   result, use a cited Civ III-specific supplementary source and label the
   evidence basis. If no suitable source is available, leave it unresolved.
3. Separate a gameplay mismatch from missing test coverage. A passing existing
   test is evidence only if its expected result traces to the manual rule.
4. Record an implementation path and a reproducible test or observation for
   each assessed entry. Do not claim full subsystem coverage from selected
   scenarios.
5. Keep implementation history and non-manual references clearly labeled.
   Supplementary Civ III sources may fill manual gaps, but do not present them
   as manual statements.

## Status values

- **Not assessed:** No manual-based comparison has been done.
- **Matches tested behavior:** The manual-supported behavior passed the linked
  test or reproducible observation.
- **Mismatch:** A test demonstrates behavior that conflicts with the supported
  manual rule.
- **Partial:** Some supported behavior was checked, while listed paths remain
  unchecked or differ.
- **Untestable from manual:** The manual does not provide a sufficiently
  specific expected result for the proposed check.
- **Intentional scope difference:** The feature is deliberately out of scope;
  record the product decision and do not describe it as a manual match.

## Audit worksheet

Add one row per rule ID from `CIV3_MANUAL_RULES.md`. Do not fill statuses until
the corresponding manual rule and implementation have actually been reviewed.

| Rule ID | Subsystem | Manual page(s) | Supplementary source ID(s) | Supported expected behavior | Implementation path | Test / evidence | Status | Gap or follow-up |
|---|---|---|---|---|---|---|---|

## Subsystem coverage tracker

Use this list to organize future work, not as a statement that these rules
have already been extracted from the manual or audited.

- [ ] Game setup and turn flow
- [ ] World, terrain, resources, and map improvements
- [ ] Cities, citizens, growth, and production
- [ ] Research and technology effects
- [ ] Governments, economy, and happiness
- [ ] Units, movement, transport, and worker actions
- [ ] Combat, healing, and special unit actions
- [ ] Diplomacy, trade, and espionage
- [ ] Wonders and other buildings
- [ ] Victory, score, and end-game behavior

## Audit record

| Date | Scope / rule IDs | Reviewer | Tests or evidence added | Summary |
|---|---|---|---|---|
