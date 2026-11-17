//! Turn, gold, government and research status shown inside the context card.

use super::*;
use crate::sim::TopBarView;

/// Vertical space the block reserves, including its bottom margin.
pub(in crate::client) const INFO_BLOCK_H: f32 = 44.0;
const CELL_H: f32 = 18.0;
const CELL_GAP: f32 = 3.0;
/// Height of the 2x2 grid itself, without margins.
pub(super) const INFO_GRID_H: f32 = 2.0 * CELL_H + CELL_GAP;

/// Label and value text for the four status cells, row by row: year and
/// government, then gold and research. The year cell has no label.
fn info_cells(view: &TopBarView) -> [(String, String); 4] {
    [
        (String::new(), year_label(view.year)),
        ("Gov".to_owned(), view.government_name.to_owned()),
        ("Gold".to_owned(), view.gold.to_string()),
        (
            view.research_name.to_owned(),
            format!("{}/{}", view.research_points, view.research_cost),
        ),
    ]
}

impl Game {
    /// Draws the 2x2 status grid at `top` inside `card`.
    pub(super) fn draw_info_block(&self, card: Rect, top: f32) {
        let Some(view) = self.sim.top_bar_view(HUMAN) else {
            return;
        };
        let muted = Color::from_rgba(181, 190, 174, 255);
        let width = card.w - 20.0;
        let cell_width = (width - 4.0) * 0.5;
        for (index, (label, value)) in info_cells(&view).iter().enumerate() {
            let cell = Rect::new(
                card.x + 10.0 + (index % 2) as f32 * (cell_width + 4.0),
                top + (index / 2) as f32 * (CELL_H + CELL_GAP),
                cell_width,
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
    fn info_cells_formats_year_government_gold_research() {
        let view = TopBarView {
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
                (String::new(), year_label(-1250)),
                ("Gov".to_owned(), "Monarchy".to_owned()),
                ("Gold".to_owned(), "310".to_owned()),
                ("Bronze Working".to_owned(), "12/30".to_owned()),
            ]
        );
    }
}
