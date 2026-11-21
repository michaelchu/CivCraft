# Ruleset configuration boundary

The native ruleset preserves campaign gameplay and saved identities while
separating editable gameplay from internal compatibility data. Runtime and
build-time BIQ metadata dependencies have been removed. The engine ruleset and
native catalogs are embedded at build time. Editing either layer requires
rebuilding the application. Live reload and a mod loader
are outside this migration.

## Ownership

| Owner | Contents |
| --- | --- |
| `data/rulesets/civcraft/content/` | Gameplay values, unit/building/technology definitions, economy, governments, AI priorities, diplomacy/strategy/vassalage/religion policies, and village reward weights |
| `data/rulesets/civcraft/engine/` | Generic effects, action enablers, unit classes, extras/resources, and remaining engine settings, using the engine's existing ruleset format |
| Rust simulation modules | Formulas, rule interpretation, state execution, validation, and turn processing |
| `data/presentation/` | Animation pacing, independent of gameplay rules |
| Campaign saves | Current health, activities, discovered technologies, research progress, owned buildings, and other mutable state |

Changing a supported value or composing an existing capability belongs in data.
Adding a new kind of mechanic, effect, action, or state requires engine support.
The migration does not introduce another gameplay scripting language.

## Units

`content/units.toml` retains costs, population costs, combat and bombardment
statistics, movement, capacity, range, support and worker values, availability,
and artwork bindings. `domain` is `Land`, `Sea`, or `Air`.

`required_technology` references a compiled technology ID, such as
`TECH_Bronze_Working`; `upgrade_to` and `replaces` reference unit IDs, such as
`swordsman`. Empty strings mean no reference. Resource requirements retain their
named resource vocabulary.

`capabilities` names supported behaviors, including `Fortify`, `FoundCity`,
`Worker`, `JoinCity`, `Paradrop`, `Embark`, `Disembark`, `Airlift`, `AttackAir`,
`IgnoreTerrain`, `Radar`, and transport/combat restrictions. The complete accepted
vocabulary is in `src/rules/unit_capabilities.rs`. `ai_roles` names existing
strategies such as `Offense`, `Defense`, `Terraform`, or `NavalTransport`.
Unknown capabilities and roles fail validation. No raw PRTO masks or imported AI
profiles are decoded. Frozen native capability and strategy expectations live in
the test fixtures.

`enslave_result` retains a native unit ID for the unfinished enslave mechanic;
its presence does not promise execution support. Unused import bytes and indices
have been removed. `UnitKind` is a data-driven index handle backed by
`content/units.toml`.

## Buildings

`content/buildings.toml` defines the active runtime improvements and wonders.
Each has a stable `id`, display name, genus, cost, upkeep, requirements,
obsolescence, and flags. The original source records and import mappings have been
removed.

Named requirement targets replace numeric references:

```toml
requirements = [{ kind = "Tech", present = true, target = "TECH_Flight" }]
obsolete = [{ kind = "Tech", present = true, target = "TECH_Motorized_Transportation" }]
```

`Improvement` and `OwnerBuildingCount` targets use building IDs. `Resource`
targets use strategic resource names. `OwnerBuildingCount` uses `count` for its
threshold; scalar requirements such as `MinSize` retain `value`.

Some inherited requirements refer to nonresearchable compatibility identities
using readable `TECH_COMPAT_<Name>` IDs (for example `TECH_COMPAT_Steam_Engine`
or `TECH_COMPAT_Railroad`). They preserve historical engine gates without adding
technologies to the research tree.

Building IDs, counts, and obsolescence policy (`selected_obsolescence`) resolve
directly from `content/buildings.toml` via data-driven `Building` handles.

## Terrain and governments

Base terrain yields, road/railroad bonuses, and movement costs live in
`src/rules/terrain_catalog.rs`, paired with `engine/terrain.ruleset` for Freeciv
extras, resources, and worker activity parameters. Civ III terrain/resource
identities and artwork mappings stay in their existing Rust adapters. `content/economy.toml`
retains campaign resource yield overrides and city foodbox/economy rules.

Government definitions and unit-support rules live in `content/governments.toml`,
backed by data-driven `Government` handles and validated against technology IDs.
An empty prerequisite allows availability without research; an unknown ID fails
catalog loading. `engine/effects.ruleset` supplies the rate cap, empire size,
and civil-war chance for every campaign government.

## Supported states

Unit activity execution remains in Rust. Whether a unit can fortify, found a
city, or perform another supported action comes from its capabilities and engine
action requirements. State modifiers use the existing effect graph: for example,
`Fortify_Defense_Bonus` and healing effects can require `Activity = Fortified`.
Building prerequisites and obsolescence are editable in the building catalog;
building-dependent effects remain in the engine effect graph.

This migration preserves these supported states. It does not add an arbitrary
state-transition editor or new building disabled/damaged states. Individual
entities' current states continue to live in campaign saves.

## Villages

`content/settlements.toml` exposes positive integer outcome weights, gold amounts,
barbarian onset, map reveal radius, and existing AI priorities. For example:

```toml
[[villages.outcomes]]
weight = 3
reward = { kind = "Gold", amount = 50 }
```

Supported reward kinds are `Gold`, `Technology`, `Reinforcements`, `Barbarians`,
`Settlement`, and `Map`. Rust executes rewards and their eligibility/fallback
rules. Weights determine the roll total; no separate roll count is editable.
The shipped order and weights preserve the original 14 rolls and random-number
consumption. Empty tables, nonpositive weights, overflowing totals, and negative
gold amounts or map radii fail validation.

## Compatibility and evidence

Native catalogs load directly from `data/rulesets/civcraft/content/*.toml`
via `src/rules/native.rs` and `src/rules/policies.rs`. Neither path joins
source records or requires BIQ files, hashes, source indices, or raw flag
encodings. Building obsolescence policy (`selected_obsolescence`) lives directly
in `content/buildings.toml`. Existing unit and technology save identities remain
unchanged.

Civilizations use stable IDs for saved identity, starting profiles and unit
availability. Display names do not control gameplay. Unit catalog race labels
match each civilization’s canonical Civ III `noun`; prototype faction aliases
are no longer supported. Frozen native technology, building, unit
capability, and obsolescence fixtures check behavior independently of import
metadata. Save tests and headless scenarios exercise the resulting simulation.
Do not update baselines merely to make a migration pass.

Civ III art files remain a separate runtime asset dependency. This change does
not replace or remove those assets.

## Religion

The owner-authorized religion extension uses
`data/rulesets/civcraft/content/religion.toml`. Like the existing custom diplomacy
policy fragments, these values are embedded at build time; rebuild after editing.
It controls influence bounds/decay, city thresholds, local and trade pressure,
policy benefits, diplomatic modifiers and change cooldowns. The typed loader
rejects unknown fields and invalid ranges. Each fixed faith entry specifies its stable ID, name, technology prerequisite
and bonus. Government does not restrict policy choices. Meditation and Divine
Right are owner-authorized custom additions in `src/rules/technologies.rs`;
their persistent IDs are appended after the existing Future/Unset sentinels.
