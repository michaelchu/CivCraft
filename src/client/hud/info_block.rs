//! Turn, gold, government and research status shown inside the context card.

use super::*;
use crate::sim::TopBarView;

/// Vertical space the block reserves, including its bottom margin.
pub(in crate::client) const INFO_BLOCK_H: f32 = 64.0;
const CELL_H: f32 = 18.0;
const CELL_GAP: f32 = 3.0;

/// Label and value text for the four status cells, in grid order. The turn
/// cell has no label.
fn info_cells(view: &TopBarView) -> [(String, String); 4] {
    [
        (
            String::new(),
            format!("{} · {}", view.turn, year_label(view.year)),
        ),
        ("Gold".to_owned(), view.gold.to_string()),
        ("Gov".to_owned(), view.government_name.to_owned()),
        (
            view.research_name.to_owned(),
            format!("{}/{}", view.research_points, view.research_cost),
        ),
    ]
}

impl Game {
    /// Draws the three-row status grid at `top` inside `card`.
    pub(super) fn draw_info_block(&self, card: Rect, top: f32) {
        let Some(view) = self.sim.top_bar_view(HUMAN) else {
            return;
        };
        let muted = Color::from_rgba(181, 190, 174, 255);
        let width = card.w - 20.0;
        let half = (width - 4.0) * 0.5;
        // (column, full width?, row): Turn and Research span the card, Gold and
        // Gov share the middle row so long names keep room.
        let layout = [(0, true, 0), (0, false, 1), (1, false, 1), (0, true, 2)];
        for ((label, value), (column, full, row)) in info_cells(&view).iter().zip(layout) {
            let cell = Rect::new(
                card.x + 10.0 + column as f32 * (half + 4.0),
                top + row as f32 * (CELL_H + CELL_GAP),
                if full { width } else { half },
                CELL_H,
            );
            draw_rectangle(
                cell.x,
                cell.y,
                cell.w,
                cell.h,
                Color::from_rgba(41, 48, 42, 255),
            );
            // Value keeps priority; the label gives up room first.
            let value = fit_text(value, 12.0, cell.w - 10.0);
            let value_size = measure_text(&value, None, 12, 1.0);
            if label.is_empty() {
                let baseline = cell.y + (cell.h - value_size.height) * 0.5 + value_size.offset_y;
                draw_text(&value, cell.x + 5.0, baseline, 12.0, WHITE);
                continue;
            }
            let label = fit_text(label, 12.0, cell.w - value_size.width - 13.0);
            let label_size = measure_text(&label, None, 12, 1.0);
            let baseline =
                |size: TextDimensions| cell.y + (cell.h - size.height) * 0.5 + size.offset_y;
            draw_text(&label, cell.x + 5.0, baseline(label_size), 12.0, muted);
            draw_text(
                &value,
                cell.right() - value_size.width - 5.0,
                baseline(value_size),
                12.0,
                WHITE,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_cells_formats_turn_year_gold_government_research() {
        let view = TopBarView {
            turn: 42,
            year: -1250,
            gold: 310,
            government_name: "Monarchy",
            anarchy: false,
            rates: [0, 60, 40],
            max_rate: 70,
            research_name: "Bronze Working",
            research_points: 12,
            research_cost: 30,
            spaceship_status: None,
            can_launch_spaceship: false,
        };
        assert_eq!(
            info_cells(&view),
            [
                (String::new(), format!("42 · {}", year_label(-1250))),
                ("Gold".to_owned(), "310".to_owned()),
                ("Gov".to_owned(), "Monarchy".to_owned()),
                ("Bronze Working".to_owned(), "12/30".to_owned()),
            ]
        );
    }
}
