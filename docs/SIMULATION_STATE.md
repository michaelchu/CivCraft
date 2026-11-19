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

## AI planning caches

`sim/ai::PlanningCaches` owns the airfield sites, continent labels and enemy city
danger reused during one AI player's planning. Its storage is private; spatial
queries take live tiles and topology, and only advisors can populate enemy danger.

Planning starts and ends alongside list-order membership in one pair of helpers.
Airfields and continent labels are built lazily and discarded when that player's
planning finishes. Queries outside planning compute fresh data without retaining
it. Enemy danger starts caching later, at the unit-movement loop, and is discarded
at the same planning end. The danger cache stores only danger, urgency and grave
threat counts; garrison defense is still recomputed as units move.

These are transient phase snapshots, excluded from saves. Cloning preserves an
active planning context with independent cache storage. Ending planning on the
clone does not clear the original. Normal turn ordering and saved AI decisions
remain unchanged.

## Advisor caches and reinforcement routes

`sim/advisors::AdvisorCaches` owns advisor activation, spatial snapshots, typed
memoizations and threat-route lifetime. Systems cannot borrow its storage maps
or change activation flags directly. Location queries preserve native tile-list
order, and ordered unit lookups fall back to the live list when its revision
changes. Research class counts are immutable shared values, so their callers
do not hold a mutable cache borrow while evaluating effects.

The advisor pass starts by rebuilding locations and clearing threat routes and
invasion support. Other memoizations retain their existing selective lifetime.
A normal finish clears all advisor and threat data. Reinforcement uses a separate
finish: it drops locations and assessments before moving friendly garrisons,
retains hostile arrival routes during those moves, then explicitly clears routes.
Threat data is private to the threat evaluator; the owner controls its lifetime.

Settlement base maps, city spacing and virtual tile yields also serve the
worker/founder phase. Its city-count and phase-end invalidations go through the
same owner and clear only those maps. Clone storage remains independent, saves
omit the entire advisor-cache group, and loading starts with it inactive.

## Visibility refresh batching

`sim/observed_map::VisibilityRefresh` owns transient deferral, pending refreshes,
dirty render tiles and the scope handed to the next refresh. Its storage is
private and separate from `ObservedMap`, which holds saved player knowledge.

Movement and exploration open conditional batches and restore the prior state
with a batch token. Nested paths retain the enclosing deferral; only returning
to an undeferred caller permits a pending flush. The owner coalesces dirty tiles
in map-index order, clears them when flushing and consumes the render scope once.
An empty scope remains distinct from an unscoped full refresh. Vision rules still
choose which tiles to mark, and the existing callers retain their flush timing.

Cloning copies pending work independently. Saving omits the batching state and
preserves remembered knowledge; loading starts with no deferral or pending work.

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

Campaign entities and durable AI memory still live directly on `Simulation`.
Their ownership can be separated in later slices without treating remembered
knowledge or scheduled decisions as disposable caches.
