# HUD Top Bar Removal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the top bar; float its buttons top-right; show Turn·Year, Gold, Government and Research in a fixed block inside the bottom-right context card; drop the unit stat boxes.

**Architecture:** `draw_top_bar` keeps only the button row (no background, title or status text). A new `info_block` module draws the four status cells from `TopBarView` and is called by `draw_context_panel` below the unit/city header (or at the card top when nothing is selected). `context_rect` heights grow to fit.

**Tech Stack:** Rust, macroquad (`src/client`).

**Spec:** `docs/superpowers/specs/2026-11-17-hud-top-bar-removal-design.md`

## Global Constraints

- Gameplay and sim code are untouched; `TopBarView` and `top_bar_view` stay as the data source.
- Unit Health/Moves/Attack/Defense boxes are removed with no replacement. Unit icon, name, owner·terrain and status line stay.
- The info block is visible for unit, city and no selection.
- No visual baseline is updated without explicit user approval.

## Review Focus

- Narrow window (about 800 px wide): floating buttons, top-left messages and the minimap must not overlap. Messages stay below the button row.
- Long government or research names must truncate inside their cell, not spill over the card edge.
- Unit whose status wraps to a second line (`inline_status == false`): the info block must start below it, not over it.
- Anarchy (`rates` forced to `[0,0,100]`) and a human player with no research target: info block still renders without panic.
- Clicks on the card or on the gaps between floating buttons: the card still blocks map clicks; the map is now clickable where the bar was.

---

### Task 1: Float the buttons and free the map

**Files:**
- Modify: `src/client/mod.rs:91` (`TOP_H`)
- Modify: `src/client/input.rs:230-236` (`viewport_rect`)
- Modify: `src/client/hud.rs:118` (message y)
- Modify: `src/client/hud/top_bar.rs` (bar background, title, status displays, tooltip and music-panel y)

**Interfaces:**
- Produces: `const CONTROLS_H: f32 = 46.0` in `src/client/mod.rs`, replacing `TOP_H`. It is the height of the floating button row. The map viewport no longer reserves it.

- [ ] **Step 1:** Replace `TOP_H` with `CONTROLS_H = 46.0`. `viewport_rect()` returns `Rect::new(0.0, 0.0, screen_width(), screen_height())`.
- [ ] **Step 2:** In `draw_top_bar`, delete the two background rectangles, the `displays` array with its measuring and drawing loop, and the "CivCraft" title. Keep `view`, the buttons, the music button and the music panel. Buttons stay at `y = 5.0`, right-aligned from `width - HUD_MARGIN`. Add a translucent dark backing per button only if the buttons are unreadable over the map in step 5; otherwise none.
- [ ] **Step 3:** Change the tooltip rect y to `button.bottom() + 3.0` and the music panel y to `CONTROLS_H + 4.0`. Change message y in `draw_messages` to `CONTROLS_H + 10.0 + ...` (unchanged position).
- [ ] **Step 4:** Run `cargo build --offline` and `grep -rn TOP_H src` (expected: no matches).
- [ ] **Step 5:** Run the game with `CIVCRAFT_SOURCE_DIR` set (see `AGENTS.md`) at 1280x720 and 800x600 (`--window-size`) and look at the top edge: buttons float over the map, messages sit below them, nothing overlaps.
- [ ] **Step 6:** Commit: `git commit -m "Float the HUD buttons and remove the top bar"`.

### Task 2: Info block in the context card

**Files:**
- Create: `src/client/hud/info_block.rs`
- Modify: `src/client/hud.rs` (add `mod info_block;`)
- Modify: `src/client/hud/context_panel.rs` (remove stat grid, call info block)
- Modify: `src/client/input.rs:238-253` (`context_rect` heights)
- Modify: `docs/UI_REVIEW.md:24-25`

**Interfaces:**
- Produces: `pub(super) const INFO_BLOCK_H: f32 = 44.0` and
  `fn info_cells(view: &TopBarView) -> [(String, String); 4]` returning (label, value) pairs in this order: `("Turn", "{turn} · {year_label(year)}")`, `("Gold", gold)`, `("Gov", government_name)`, `(research_name, "{points}/{cost}")`.
- Produces: `impl Game { pub(super) fn draw_info_block(&self, card: Rect, top: f32) }` drawing the 2×2 grid at `card.x + 10.0, top`, each cell 18 px high with a 3 px row gap and the same cell colors, label (muted, left, `fit_text`) and value (white, right) styling the removed stat grid used. Does nothing if `top_bar_view(HUMAN)` is `None`.
- Consumes: `TopBarView` (`src/sim/client_views/mod.rs:57`), `year_label`.

- [ ] **Step 1:** Write failing test `info_cells_formats_turn_year_gold_government_research` in `info_block.rs`. Build a `TopBarView` literal (turn 42, year -1250, gold 310, government `"Monarchy"`, research `"Bronze Working"` 12/30). Assert the four pairs equal `("Turn", format!("42 · {}", year_label(-1250)))`, `("Gold","310")`, `("Gov","Monarchy")`, `("Bronze Working","12/30")`.
- [ ] **Step 2:** Run `cargo nextest run --profile ci info_cells` (expected: FAIL, function not defined).
- [ ] **Step 3:** Implement `info_cells` and `draw_info_block` as specified above.
- [ ] **Step 4:** Run the same command (expected: PASS).
- [ ] **Step 5:** In `context_panel.rs`, delete the `metrics` grid and call `self.draw_info_block(card, top)` in its place, where `top` is the grid's old `grid_y` (`card.y + 44.0` inline, else `card.y + 56.0`). Unit card height stays 140.
- [ ] **Step 6:** City view: call `draw_info_block(card, y + 54.0)` below the "City size" line and shift the following lines by `INFO_BLOCK_H + 4.0`; include that offset in the `spacing` divisor (`156.0` becomes `156.0 + INFO_BLOCK_H + 4.0`).
- [ ] **Step 7:** No selection: in `draw_context_panel`, add an `else` branch that calls `draw_info_block(card, card.y + 6.0)`.
- [ ] **Step 8:** `context_rect` heights: unit `140.0`, city `174.0 + INFO_BLOCK_H + 4.0`, none `54.0 + INFO_BLOCK_H + 4.0`. Verify the End Turn button and city lines do not collide.
- [ ] **Step 9:** Update the two `docs/UI_REVIEW.md` rows: "Top bar" becomes floating icon buttons in the upper right; the map fills the whole window; status info is in the context card.
- [ ] **Step 10:** Run `cargo nextest run --profile ci --all-targets` (expected: all pass).
- [ ] **Step 11:** If `test -d "$CIVCRAFT_SOURCE_DIR/Conquests/Art/Terrain"` passes, run the game or `--ui-review` into a new output directory with no baseline, and view the unit, wrapped-status unit, city and empty-selection cards at 1280x720 and 800x600. Otherwise report visual review as skipped. Do not update any baseline.
- [ ] **Step 12:** Commit: `git commit -m "Show turn, gold, government and research in the context card"`.
