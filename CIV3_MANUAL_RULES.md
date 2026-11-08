# Civilization III manual rules catalog

Status: **Not yet populated.** This document is the future source of expected
gameplay behavior derived from [`civ3_manual.pdf`](civ3_manual.pdf). No
manual-to-code audit or gameplay gap assessment has been performed as part of
creating this template.

## Authority and interpretation

- Use the supplied manual as the authority for the intended Civilization III
  rule wherever it states the rule.
- Record the manual page for every rule. Include a short quotation only when
  necessary; otherwise paraphrase the relevant passage.
- Do not infer an exact formula, timing detail, exception, or random procedure
  from a general description. Record what the manual says and mark missing
  detail as unspecified.
- If a rule depends on an edition, expansion, difficulty, or game setting,
  record that scope. Do not silently mix base-game and expansion behavior.
- When the manual does not specify a detail needed for an expected result, use
  a clearly identified Civ III-specific supplementary source if available.
  Prefer original game materials and official data, then well-sourced
  specialist references. Record the exact claim each source supports and any
  disagreement between sources.
- Do not treat Freeciv behavior, implementation details, or existing tests as
  Civ III evidence. They may be noted separately as implementation history,
  never as the expected result for a manual-based rule.

## Source records

Keep one entry per source in the source list. Link each rule to the source
records that support it. Preserve edition, release, and expansion scope; a
source that mixes versions must be checked before its data is used.

```text
Source ID:
Title / publisher or author:
URL or local file:
Source type: Original game material | Official publication | Specialist reference | Other
Edition / expansion / patch scope:
Pages, table, figure, or section:
Retrieved or checked date:
Claims this source supports:
Limitations or conflicts:
```

### Technology-tree source leads

These are candidate references for details not fully tabulated in the
supplied manual. Their claims have not yet been reconciled into rule entries.

- **TECH-WIKI — Vi.ki, “Civ 3 Tech Tree: All Advances, Eras and Optional
  Techs”**: <https://civ3.vi.ki/technologies>. The page says its base-game
  counts come from Prima's official strategy guide and cross-references both
  the 2001 manual and the Complete manual. Treat it as a sourced secondary
  reference; verify table details and version scope against the cited source
  where possible.
- **TECH-CF-TREE — CivFanatics, “Civilization III: Tech Tree”**:
  <https://civfanatics.com/civ3/tech-tree/>. An interactive era-by-era tree;
  verify the displayed version and capture prerequisite details when
  transcribing it.
- **TECH-CF-PDF — Nethog's Civ 3 Tech Tree, Rev 1.00 (29 Oct 2002)**:
  <https://www.civfanatics.net/downloads/civ3/reference/civ3_tech_tree_rev100.pdf>.
  A diagram with technology nodes and prerequisite arrows. Its version and
  expansion applicability must be checked before using it as a base-game
  oracle.
- **TECH-CF-INFO — CivFanatics Civilization III Info Center**:
  <https://civfanatics.com/civ3/infocenter/>. A broad community index that
  gathers information from mixed source types; use as a discovery aid, then
  trace gameplay claims to a more specific source.

The online sources do not fully agree on the reported base-game technology
count: the Vi.ki page reports 82 based on its count of Prima's table and also
mentions an 81-tech count in a Firaxis comparison; CivFanatics reports 82.
Record this as a source discrepancy to resolve during the technology audit,
not as a settled manual-derived fact.

### General gameplay-mechanics index

- **VI-PAGES — Vi.ki, “Every page — Civilization 3”**:
  <https://civ3.vi.ki/pages>. This directory links detailed pages for core
  mechanics including cities, combat, corruption, culture, diplomacy,
  governments, resources, terrain/workers, units, technologies, and victory.
  Treat it as a secondary discovery index: the linked articles cite a mixture
  of manuals, Prima's guide, previews, and archived official material. Follow
  the article's source links and cite the specific underlying source for each
  rule where possible; check base-game versus expansion scope.
- **VI-COMBAT — Vi.ki, “Combat”**:
  <https://civ3.vi.ki/combat>. A worked mechanics summary with a source list;
  use it to locate underlying sources, and validate detailed formulas against
  those sources before treating them as expected values.
- **VI-CITIES — Vi.ki, “Cities”**: <https://civ3.vi.ki/cities>. A city
  mechanics summary whose claims cite specific manual editions and Prima page
  ranges; preserve those edition distinctions when following a claim.

## Confidence labels

- **Explicit:** The manual directly states the behavior, value, condition, or
  relationship needed for the claim.
- **Inferred:** The claim follows from the manual's description, but some
  details are not stated. Limit tests to the observable consequence that is
  supported by the text and explain the inference.
- **Unspecified:** The manual does not provide enough detail to set an expected
  result. Do not create a parity assertion for this claim.

## Rule entry template

Copy this block for each distinct rule. Keep one independently testable
behavior per entry.

```text
Rule ID:
Subsystem:
Rule title:
Manual edition / scope:
Manual page(s):
Manual statement (paraphrase):
Confidence: Explicit | Inferred | Unspecified
Supported expected behavior:
Details the manual leaves unspecified:
Supplementary source ID(s):
Related manual rules:
```

## Rule entries

No rules have been transcribed or assessed yet.
