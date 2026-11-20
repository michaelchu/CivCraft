# Culture as a core CivCraft mechanic

Status: implementation in progress. Date: 2026-11-19.

## Intent

Culture should be a visible long-term strategy and a normal route to victory in
every CivCraft campaign. The standard ruleset may tune building effects and
migration, but it must not disable cultural victory. Civ III Conquests supplies
the culture model for this explicit gameplay exception to the Freeciv port.

## Decisions

- A city's culture is cumulative. Its culture-producing buildings add points
  each city turn. Each building's rate doubles once it has existed for 1,000
  campaign years. Culture age is tracked per building and survives saves.
- Local city culture is the only source of border growth (city size no longer
  widens it; the ruleset's `radius_sq_city`/`size_effect` are unused). A new
  city claims the 3×3 square (radius 1). At the Civ III thresholds 10, 100,
  1,000, and 10,000 each side extends one tile outward and the corners are
  trimmed: the 21-tile "fat X" (equal to the city work radius), then 45, 77 and
  117 tiles. The shape is `MapWrap::civ3_border_shape_contains`, evaluated on
  CivCraft's Civ III map axes. Existing C2C3 tile claimability still resolves
  which tiles an expanded border can claim. Workers need owned tiles, so a city
  works only its eight neighbors until it reaches 10 culture.
- Civilization culture is permanent. When a city is lost or destroyed, its
  accumulated local culture moves into its former owner's national culture
  history. A captured city starts a fresh local culture history. An occupier
  gets no culture from buildings in the captured city; the original builder
  resumes production if they recapture it, without the old age bonus.
- Cultural victory is always evaluated at the normal game-over boundary. It is
  won either by one city reaching 20,000 culture, or by an empire reaching a
  100,000-point standard-map target scaled to the campaign map area and having
  at least twice the culture of the runner-up. A rival at exactly half does
  not block victory, matching the Conquests wording.
- Culture rates are stored as `culture_per_turn` in the native building data.
  The seven cultural improvements and Wonders use the Civilization III
  Complete manual's Conquests-era rates. Freeciv `History` effects no longer
  determine core culture output. The manual's Small Wonder table does not list
  the Conquests-added Secret Police HQ; it remains at zero pending a direct
  BIQ check. C2C3-only Wonder names absent from the Conquests roster have zero
  culture in the core model.
- Culture flipping is deferred. It needs dedicated probability tuning and
  player-facing warnings before it can safely alter city ownership.

## Architecture

- Turn processing owns culture accrual at the existing city-history boundary.
- The shared border rebuild consumes city culture tiers; it retains the
  existing requirement evaluator, map topology, and worker reconciliation.
- Persistent building-age records are appended in a new save format rather
  than changing frozen entity wire layouts. Older saves receive neutral age
  records for their existing buildings.
- The city manager displays current culture, culture per turn, and the next
  border milestone. The campaign victory check reads live culture, not the
  previous turn's score snapshot.
- Add the explicit Civ III culture exception to `AGENTS.md` and update the
  culture/border row in `FREECIV_GAMEPLAY_PORT_PLAN.md`.

## Audited rates

Rates are culture points per turn before age or culture-percentage bonuses.
They are stored by building name in `data/rulesets/civcraft/content/buildings.toml`.
The Civilization III Complete manual's table on page 147 is the primary source.

| Cultural improvement | Rate |
| --- | ---: |
| Cathedral | 3 |
| Colosseum | 2 |
| Library | 3 |
| Palace | 1 |
| Research Lab | 2 |
| Temple | 2 |
| University | 4 |

| Great Wonder | Rate | Great Wonder | Rate |
| --- | ---: | --- | ---: |
| Adam Smith's Trading Company | 3 | Colossus | 3 |
| Copernicus' Observatory | 4 | Cure for Cancer | 5 |
| Great Library | 5 | Great Lighthouse | 2 |
| Great Wall | 2 | Hanging Gardens | 4 |
| Hoover Dam | 3 | Internet | 4 |
| J.S. Bach's Cathedral | 5 | Knights Templar | 2 |
| Leonardo's Workshop | 2 | Longevity | 3 |
| Magellan's Expedition | 3 | Manhattan Project | 2 |
| Michelangelo's Chapel | 4 | Mausoleum of Mausolos | 2 |
| Newton's University | 5 | Oracle | 4 |
| Pyramids | 4 | SETI Program | 3 |
| Shakespeare's Theater | 5 | Statue of Zeus | 4 |
| Sun Tzu's War Academy | 2 | Temple of Artemis | 4 |
| Theory of Evolution | 3 | United Nations | 4 |
| Universal Suffrage | 4 |  |  |

| Small Wonder | Rate | Small Wonder | Rate |
| --- | ---: | --- | ---: |
| Apollo Program | 3 | Battlefield Medicine | 1 |
| Forbidden Palace | 3 | Heroic Epic | 4 |
| Intelligence Agency | 1 | Iron Works | 2 |
| Military Academy | 1 | Pentagon | 1 |
| Strategic Missile Defense | 1 | Wall Street | 2 |
| Secret Police HQ | 0 |  |  |

`Secret Police HQ` is absent from the printed manual's small-wonder table; the
Conquests additions list at CivFanatics gives it 0 culture. The BIQ itself is
not available in this workspace for a direct field check. `Ecclesiastical
Palace` retains a 1-point adapter rate from the existing selected ruleset; it
is not in the standard Conquests Wonder roster. Other C2C3-only Wonder names
without a Conquests identity are set to 0 rather than inheriting Freeciv
`History` output.

## Out of scope

- Culture flipping, diplomatic culture attitude, and culture-driven citizen
  assimilation.
- A separate culture advisor screen or new artwork.
- Scenario-specific culture victory switches.

## References

- [Civilization III Complete manual](https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/3910/manuals/manual.pdf), culture table on p. 147 and Conquests Wonders on pp. 132-133.
- [Prima's Civilization III strategy guide](https://device.report/m/d76e460361da693c0ec91ddff041d42d7a117f67b42cd55f945543fbd819c27a.pdf), culture values and border thresholds.
- [CivFanatics culture FAQ](https://civfanatics.com/civ3/infocenter/), capture behavior and border thresholds.
- [CivFanatics Conquests changes list](https://forums.civfanatics.com/threads/civ3-conquests-additions-changes-list.104294/), rates of the five Conquests-added Wonders, including Secret Police HQ.
- Dan Magaha (Firaxis), “Culture-Flipping Exposed,” source explanation of the
  Civ III conversion factors.
- Civilization III: Conquests FAQ, map-size cultural victory values.
