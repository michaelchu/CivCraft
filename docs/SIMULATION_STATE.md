# Simulation state and derived caches

`Simulation` owns the live campaign. Its systems currently share that state;
cache extraction is incremental, with each owner defining its keys and lifetime.

## Query caches

`sim/caches::QueryCaches` owns reconstructible query memoizations. Its storage is
private. Simulation systems use typed lookups rather than borrowing the cache maps.
Research and movement cache implementations are private children of their rule
modules; only those modules can insert the corresponding computed results.

| Cache | Validity and invalidation |
| --- | --- |
| Unit and city row positions | A cached position is checked against the current collection and persistent ID on every lookup. Reordering, removal and insertion repair stale hints lazily. Missing IDs are not permanently cached. |
| Research eras and prerequisite paths | Each owner's exact known-technology list is the key. A change clears both era and path results together. |
| Movement nativeness and step costs | Keys include the complete movement tile descriptors, unit class and applicable policy flags. Rule data is immutable for the campaign process; changed inputs select different entries. |

Cloning copies the memoizations independently, preserving the previous clone
behavior. They are omitted from the explicit save schema. Loading a campaign
clears this group after validation and restoration; later queries repopulate it.
Cache population or clearing must preserve encoded campaign bytes and future
turn results.

## Durable knowledge and scheduled decisions

`observed_map` contains remembered player knowledge and remains in campaign
persistence. Reconstructing it from the live world would reveal hidden information.
Its derived rendering indexes retain their existing restoration path.

A field named "cache" is not automatically disposable. For example,
`ai_government_cache` includes the advisor's evaluation schedule and is explicitly
saved. AI memories, strategic commitments and list ordering also retain their
existing persistence contracts.

The trace collector already owns transient instrumentation and lifecycle/research
ledgers. It remains separate from both query memoization and saved knowledge.

AI planning/spatial caches and visibility batching still live on `Simulation`.
Their phase boundaries and invalidation rules should be extracted in subsequent
slices, preserving phase order, deterministic decisions and save compatibility.
