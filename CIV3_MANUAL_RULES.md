# Civilization III manual rules catalog

Status: **Terrain entries started.** The entries below capture the water and
special land terrain facts used in the current map model. A broader
manual-to-code audit remains outstanding.

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

### CIV3-MANUAL

- Source ID: CIV3-MANUAL
- Title / publisher: *Civilization III: The Official Manual*, Firaxis Games
- URL or local file: [`civ3_manual.pdf`](civ3_manual.pdf), project root
- Source type: Original game material
- Edition / expansion / patch scope: Base Civilization III manual, 2001
- Pages, table, figure, or section: pp. 206–207, terrain yield and resource tables
- Retrieved or checked date: 2026-11-08
- Claims this source supports: Coast, Sea, and Ocean base yields and resource
  habitats; Flood Plains and shield yielding Grassland yields
- Limitations or conflicts: Does not specify procedural map-generation
  thresholds, the frequency of shielded Grassland, or Conquests-only Volcano
  and Marsh rules

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

### TERRAIN-WATER-OUTPUTS

- Subsystem: Terrain and city tile output
- Rule title: Coast, Sea, and Ocean base yields
- Manual edition / scope: Base Civilization III
- Manual page(s): 206
- Manual statement (paraphrase): Coast yields 1 food and 2 commerce; Sea yields
  1 food and 1 commerce; Ocean yields 1 food and no commerce. These terrains
  yield no base shields.
- Confidence: Explicit
- Supported expected behavior: Model distinct Coast, Sea, and Ocean terrain
  types with base yields `(1, 0, 2)`, `(1, 0, 1)`, and `(1, 0, 0)`.
- Details the manual leaves unspecified: Map-generation depth thresholds and
  how the generator chooses each water type.
- Supplementary source ID(s): None
- Related manual rules: TERRAIN-WATER-RESOURCES

### TERRAIN-WATER-RESOURCES

- Subsystem: Resource placement
- Rule title: Fish and Whale water habitats
- Manual edition / scope: Base Civilization III
- Manual page(s): 207
- Manual statement (paraphrase): Fish may appear on Coast, Sea, and Ocean;
  Whales may appear on Sea and Ocean.
- Confidence: Explicit
- Supported expected behavior: Permit Fish on Coast, Sea, and Ocean, and
  Whales on Sea and Ocean. Deep Ocean is not excluded.
- Details the manual leaves unspecified: Relative spawn frequency and placement
  spacing.
- Supplementary source ID(s): None
- Related manual rules: TERRAIN-WATER-OUTPUTS

### TERRAIN-FLOOD-PLAINS

- Subsystem: Terrain and city tile output
- Rule title: Flood Plains yield
- Manual edition / scope: Base Civilization III
- Manual page(s): 206
- Manual statement (paraphrase): Flood Plains produce 3 food before
  improvements.
- Confidence: Explicit
- Supported expected behavior: Model Flood Plains as a 3-food terrain variant.
- Details the manual leaves unspecified: The terrain table does not specify a
  map-generation algorithm or placement frequency.
- Supplementary source ID(s): None
- Related manual rules: None

### TERRAIN-GRASSLAND-SHIELD

- Subsystem: Terrain and city tile output
- Rule title: Shield yielding Grassland variant
- Manual edition / scope: Base Civilization III
- Manual page(s): 37, 206
- Manual statement (paraphrase): Some Grassland squares have a rocky-tuft symbol
  and produce one shield in addition to the normal 2 food. The terrain table
  also lists +1 food from irrigation, +1 shield from mining, and +1 commerce
  from roads on Grassland.
- Confidence: Explicit
- Supported expected behavior: Represent shield yielding Grassland separately
  from ordinary Grassland and give it 2 food and 1 shield. Apply the normal
  Grassland irrigation, mine, and road bonuses to this variant.
- Details the manual leaves unspecified: How often map generation places this
  variant and the exact sprite used for its rocky-tuft symbol.
- Supplementary source ID(s): None
- Related manual rules: None
