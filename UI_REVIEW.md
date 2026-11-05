# UI structure and visual review

The client uses Macroquad's immediate-mode renderer. View files own their screen-specific content; `src/client/ui.rs` owns shared pixel-measured text fitting, panel framing, dialog text bounds, gutters, grid sizing and pagination. Buttons and meters remain shared primitives in `src/client/mod.rs`.

This is deliberately a small shared UI layer rather than a retained widget framework. Screen-specific game state and graph rendering remain in their existing modules. Some view coordinates still use local offsets; the UI is more consistent but is not completely free of duplicated layout code.

## Reviewed surfaces

| Surface | Cleanup |
| --- | --- |
| Top bar | Distinct turn, government, diplomacy, treasury and research regions; measured text bounds prevent collisions. |
| Sidebar | Seed gets its own row; context text stays inside the column; compact windows use tighter context spacing; rate controls share equal gutters. |
| Action strip | Two rows and a More control expose overflow actions instead of dropping them. |
| City manager | Wider responsive modal; separate name, forecast, progress and specialist rows; production grid adapts rows and columns to available space. |
| Production cards | Names, cost and status fit their cards; pagination uses the actual grid capacity. |
| Worklist / worked tiles | Queue labels leave space for editing controls. Worked tiles use an isometric map with shared world terrain, improvements and city artwork; surrounding tiles are fogged, with diamond-shaped worker hit targets. |
| Government | Common frame, bounded prose, existing two-column government choices. |
| Diplomacy | Rival details sit above wrapping action rows; map and city offers remain available in narrower windows. |
| City deals / treaties | Names stay clear of purchase controls; treaty columns fit independently and reserve footer space. |
| Spy / investigation | Common frame, bounded titles and content, shared Previous/Next footer with page count. |
| Research tree | Pixel-bounded summaries and details, consistent tab gutters; tree drawing is unchanged. |
| Spaceship | Common frame and bounded prose; slot labels use supported font glyphs. |

The city manager now captures modal input and removes underlying HUD hitboxes. This prevents clicks on covered HUD controls.

## Reproduce visual QA

From the app directory:

```sh
cargo build --offline --release
./target/release/civcraft --seed 42 --ui-review /tmp/civcraft-ui-review --window-size 1280x720
```

The gallery renders 14 isolated scenarios with actual game assets and the production renderer. It uses long city names, a large treasury and a full-width seed, populates diplomacy and spy dialogs, exercises available next-page actions, and checks that actionable controls remain inside the window. Each scenario produces a PNG plus a second PNG after pagination. The fixture does not affect normal games and disables autosaving.

Repeat at 800x600 and 1600x900 to inspect compact and wide layouts. Run the existing desktop interaction suite separately:

```sh
./target/release/civcraft --seed 42 --self-test /tmp/civcraft-ui-interactions
cargo test --offline
```

Screenshots still require visual inspection: control bounds alone cannot prove text readability or pleasing spacing. Windows smaller than 800x600 are outside this review's tested range.

## Validation on 2026-11-05

- 488 Rust tests passed.
- 278 native desktop interaction checks passed, after updating the script to close the city modal before using the HUD and to navigate action pages.
- Inspected gallery renders at 800x600, 1280x720 and 1600x900; actionable-control window bounds passed at each size.
- Formatting and whitespace checks passed.

Parity implementation remains paused during this UI work.

### Isometric worked-tile view

The city map reuses the world's terrain and improvement passes. It fits the city radius into the available panel space, clips terrain overhangs, and fogs irrelevant or unexplored tiles. Gold outlines show the city center and worked tiles, with each shared edge drawn once at a consistent thickness; tile yield labels remain visible. Click targets follow each diamond rather than its overlapping bounding rectangle. Preview rendering does not move or zoom the main world camera.

Validation: 490 Rust tests and 278 desktop checks passed, including worker/specialist reassignment. Standard and compact window captures passed control-bound checks; the main-world screenshot remained pixel-identical after extracting the shared terrain pass.
