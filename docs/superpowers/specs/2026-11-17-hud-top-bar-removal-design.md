# HUD redesign: remove the top bar

## Intent
The top bar spends a full-width strip on a title and status text. Free the map
area by removing it, and keep the status info visible in the bottom-right
overlay.

## Goals
- No top bar (no background strip, no "CivCraft" title).
- The bar's buttons float top-right over the map, unchanged in behavior:
  Government, Diplomacy, Espionage, Research tree, Science/Tax/Luxury rates,
  Spaceship (when available), Music. Tooltips and the music panel are kept.
- Bottom-right context card gains a fixed info block directly below the
  header section at the top of the card (the unit icon, name and status, or the
  city name): Turn·Year, Gold, Government, Research progress. It uses the 2x2
  cell style the unit stats used, in the spot those boxes occupied.
- The info block is always visible: unit selected, city selected, or nothing.

## Non-goals
- No change to panel contents, rates logic, or the city detail view.
- No new keyboard shortcuts or menu.

## Anti-goals
- Overlapping the minimap, messages, or map edge after `TOP_H` shrinks.

## Decisions
- Unit Health/Moves/Attack/Defense boxes are removed with no replacement
  (the user's call; the unit name and status line remain). Noted risk: moves
  left and HP are no longer shown anywhere.
- Research progress joins the info block; the title is dropped.

## Approaches considered
1. Info block below the header of the existing overlay card (chosen): smallest
   change, same behavior for every selection.
2. Separate stacked card above the context card: cleaner, but a second card
   to place around the minimap and messages.
3. One-line strip: most compact, cramped at narrow widths, no room for
   research progress.

## Left to the builder
- Exact card height and spacing; narrow-width truncation of long government
  or research names.
- Where `TOP_H` is used (message placement in `hud.rs`, music panel in
  `top_bar.rs`, map input hit-testing, UI-review scenarios) and adjusting each.
- Updating `--ui-review` expectations; baselines are not updated without
  explicit approval.

## Files likely touched
`src/client/hud/top_bar.rs`, `src/client/hud.rs`,
`src/client/hud/context_panel.rs`.
