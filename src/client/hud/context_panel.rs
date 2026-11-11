//! Context-sensitive controls for the currently selected city or unit.

use super::*;

impl Game {
    pub(super) fn draw_context_panel(&self, assets: &Assets, card: Rect, button_y: f32) {
        let x = card.x + 10.0;
        let y = card.y + 4.0;
        let spacing = ((button_y - y - 6.0) / 156.0).clamp(0.0, 1.0);
        let context_text = |text: &str, tx: f32, ty: f32, size: f32, color: Color| {
            text_in(
                text,
                tx,
                y + (ty - y) * spacing,
                size.min(17.0),
                card.w - 20.0,
                color,
            );
        };
        if let Some(unit_id) = self.selected_unit {
            let animated = self
                .playback
                .active()
                .into_iter()
                .map(|(unit, _)| unit)
                .find(|shown| shown.id == unit_id);
            let view = animated.as_ref().map_or_else(
                || self.sim.unit_context_view_by_id(unit_id),
                |unit| self.sim.unit_context_view_for_rendered(unit),
            );
            let Some(view) = view else {
                return;
            };
            assets.civ3.draw_unit_icon(
                view.kind,
                view.owner,
                view.civilization_id.as_deref(),
                view.era_index,
                vec2(x + 18.0, card.y + 21.5),
                vec2(36.0, 27.0),
                view.moves_left > 0,
            );

            let width = card.w - 20.0;
            let right = x + width;
            let identity_x = x + 41.0;
            let muted = Color::from_rgba(181, 190, 174, 255);
            let status_color = Color::from_rgba(188, 219, 175, 255);
            let state = view.status;
            let state_width = measure_text(&state, None, 12, 1.0).width;
            let name_width = measure_text(view.kind.name(), None, 14, 1.0).width;
            let inline_status = identity_x + name_width + state_width + 8.0 <= right;
            let title_y = card.y + 20.0;
            text_in(
                view.kind.name(),
                identity_x,
                title_y,
                14.0,
                right - identity_x,
                Color::from_rgba(244, 221, 147, 255),
            );
            text_in(
                &format!("{} · {}", view.owner_name, view.terrain_name,),
                identity_x,
                title_y + 14.0,
                12.0,
                right - identity_x,
                muted,
            );
            if inline_status {
                draw_text(&state, right - state_width, title_y, 12.0, status_color);
            } else {
                text_in(&state, x, title_y + 29.0, 12.0, width, status_color);
            }

            let gap = 4.0;
            let cell_width = (width - gap) * 0.5;
            let moves = view.moves_allowance;
            let metrics = [
                ("Health", format!("{}/{}", view.hp, view.max_hp)),
                ("Moves", format!("{}/{moves}", view.moves_left)),
                ("Attack", view.attack.to_string()),
                ("Defense", view.defense.to_string()),
            ];
            for (index, (label, value)) in metrics.iter().enumerate() {
                let cell_x = x + (index % 2) as f32 * (cell_width + gap);
                let grid_y = if inline_status {
                    card.y + 44.0
                } else {
                    card.y + 56.0
                };
                let cell_y = grid_y + (index / 2) as f32 * 21.0;
                let cell = Rect::new(cell_x, cell_y, cell_width, 18.0);
                draw_rectangle(
                    cell.x,
                    cell.y,
                    cell.w,
                    cell.h,
                    Color::from_rgba(41, 48, 42, 255),
                );

                let value_size = measure_text(value, None, 12, 1.0);
                let label = fit_text(label, 12.0, cell.w - value_size.width - 13.0);
                let label_size = measure_text(&label, None, 12, 1.0);
                let baseline =
                    |size: TextDimensions| cell.y + (cell.h - size.height) * 0.5 + size.offset_y;
                draw_text(&label, cell.x + 5.0, baseline(label_size), 12.0, muted);
                draw_text(
                    value,
                    cell.right() - value_size.width - 5.0,
                    baseline(value_size),
                    12.0,
                    WHITE,
                );
            }
        } else if let Some(city_id) = self.selected_city
            && let Some(city) = self.sim.city_context_view(city_id)
        {
            context_text(
                &city.name,
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            context_text(
                &format!(
                    "City size {} ({} foreign)  Plague {:.1}%",
                    city.size,
                    city.foreign_citizens,
                    f64::from(city.illness) / 10.0
                ),
                x,
                y + 48.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!(
                    "Food {}  Shields {}  Trade {}",
                    city.food, city.shields, city.trade
                ),
                x,
                y + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            context_text(
                &format!("Food store {}/{}", city.food_store, city.growth_box),
                x,
                y + 102.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!("Producing: {}", city.production_name),
                x,
                y + 132.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            context_text(
                &format!(
                    "Production {}/{}",
                    city.production_stock, city.production_cost
                ),
                x,
                y + 156.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
        }
    }
}
