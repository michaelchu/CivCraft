# Refactor duplication inventory

Initial inventory for the modularity and DRY refactor. This is a conservative
classification: similar structure is not treated as duplication unless both
the source and behavior match.

## Findings

| Area | Classification | Decision |
| --- | --- | --- |
| `rules/buildings.rs` and `rules/research.rs` catalog declarations | Similar schema, distinct domains | Keep separate. Their entries, source identities, and lookup behavior are not interchangeable. A shared catalog framework would add abstraction without removing duplicated behavior. |
| Conquests rules and legacy/compatibility values | Intentional variants | Keep source-specific values and ordering separate. Do not merge entries solely because names or fields look alike. |
| `client/ui.rs` shared panel, text-fitting, grid, and pagination helpers | Already consolidated | Reuse these helpers; do not create a second generic widget/layout layer. |
| `sim/ui_review.rs` terrain/map scene setup helpers | Intentional shared fixture composition | Keep shared setup helpers, but isolate them under a scenario module rather than treating scenario calls as duplicate gameplay logic. |
| Client UI event messages and simulation event log | Similar output destination, different ownership | Separate transient client notices from persisted gameplay events. Do not route simulation outcomes through a client-only queue. |
| Map-generation tile and campaign tile records | Confirmed representation coupling, not duplicate behavior | Give generation its own compact tile result and convert at campaign construction. Keep shared terrain/resource domain enums. |

## Consolidation rule

No gameplay rule or catalog data is consolidated in this pass. Follow-up
consolidation requires evidence that records share provenance and produce the
same behavior; edition-specific and compatibility values remain distinct.
