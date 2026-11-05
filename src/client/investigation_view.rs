use super::*;

impl Game {
    pub(super) fn draw_investigation(&self, report: &CityInvestigation) -> Vec<(Rect, UiAction)> {
        let city = &report.city;
        let forecast = &report.forecast;
        let width = (screen_width() - 24.0).min(620.0);
        let height = (screen_height() - 24.0).clamp(220.0, 440.0);
        let panel = Rect::new(
            (screen_width() - width) / 2.0,
            (screen_height() - height) / 2.0,
            width,
            height,
        );
        draw_rectangle(
            panel.x,
            panel.y,
            panel.w,
            panel.h,
            Color::from_rgba(42, 44, 40, 255),
        );
        draw_rectangle_lines(panel.x, panel.y, panel.w, panel.h, 2.0, GOLD);
        let x = panel.x + 18.0;
        draw_text(
            &format!("Investigation of {}", city.name),
            x,
            panel.y + 31.0,
            23.0,
            WHITE,
        );
        draw_text(
            &format!(
                "{}  ·  Size {}  ·  Food stock {}",
                self.sim.players()[city.owner].name,
                city.size,
                city.food
            ),
            x,
            panel.y + 57.0,
            17.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!(
                "Producing {}  ·  {} / {} shields",
                city.production.name(),
                city.shields,
                city.production.cost()
            ),
            x,
            panel.y + 82.0,
            17.0,
            LIGHTGRAY,
        );
        draw_text(
            &format!(
                "Food {}  ·  Shields {}  ·  Trade {}  ·  Science {}  ·  Gold {}",
                forecast.food, forecast.shields, forecast.trade, forecast.science, forecast.gold
            ),
            x,
            panel.y + 106.0,
            16.0,
            LIGHTGRAY,
        );
        let close = Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0);
        draw_ui_button(close, "X", true, false);
        let mut actions = vec![(close, UiAction::CloseInvestigation)];
        let mut lines = Vec::new();
        lines.push(format!("Buildings ({})", city.buildings.len()));
        lines.extend(
            city.buildings
                .iter()
                .map(|building| format!("    {}", building.data().name)),
        );
        lines.push(format!("Stationed units ({})", report.stationed.len()));
        lines.extend(
            report
                .stationed
                .iter()
                .map(|unit| format!("    {}  ·  {} HP", unit.kind.name(), unit.hp)),
        );
        lines.push(format!("Supported units ({})", report.supported.len()));
        lines.extend(report.supported.iter().map(|unit| {
            format!(
                "    {}  ·  {} HP  ·  {}, {}",
                unit.kind.name(),
                unit.hp,
                unit.x,
                unit.y
            )
        }));
        lines.push(format!("Worklist ({})", city.worklist.len()));
        lines.extend(
            city.worklist
                .iter()
                .map(|target| format!("    {}", target.name())),
        );
        let rows = ((panel.h - 166.0) / 19.0).floor().max(1.0) as usize;
        let last = (lines.len() - 1) / rows;
        let page = self.investigation_page.min(last);
        for (row, line) in lines.iter().skip(page * rows).take(rows).enumerate() {
            draw_text(line, x, panel.y + 135.0 + row as f32 * 19.0, 16.0, WHITE);
        }
        for (enabled, left, label, target) in [
            (page > 0, x, "Previous", page.saturating_sub(1)),
            (page < last, panel.right() - 96.0, "Next", page + 1),
        ] {
            if enabled {
                let button = Rect::new(left, panel.bottom() - 36.0, 78.0, 25.0);
                draw_ui_button(button, label, true, false);
                actions.push((button, UiAction::InvestigationPage(target)));
            }
        }
        actions
    }
}
