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
        if let Some(index) = self.selected_unit_index() {
            let unit = &self.sim.units()[index];
            let animated = self
                .playback
                .active()
                .into_iter()
                .map(|(unit, _)| unit)
                .find(|shown| shown.id == unit.id);
            let unit = animated.as_ref().unwrap_or(unit);
            let stats = unit.kind.stats();
            let state = if unit.work_left > 0 {
                format!(
                    "Building {} - {} turns",
                    unit.work_kind.map(|w| w.name()).unwrap_or("improvement"),
                    self.sim.project_turns_left(unit)
                )
            } else if let Some(carrier) = unit.transport {
                self.sim
                    .units()
                    .iter()
                    .find(|ship| ship.id == carrier)
                    .map_or_else(
                        || "Aboard transport".to_owned(),
                        |ship| format!("Aboard {}", ship.kind.name()),
                    )
            } else if unit.kind.transport_capacity() > 0 {
                format!(
                    "Cargo {}/{}",
                    self.sim.cargo_count(unit.id),
                    unit.kind.transport_capacity()
                )
            } else if unit.automated {
                "Automated".to_owned()
            } else {
                match unit.state {
                    UnitState::Idle => "Ready".to_owned(),
                    UnitState::Fortifying => "Fortifying".to_owned(),
                    UnitState::Fortified => "Fortified (+50% defense)".to_owned(),
                    UnitState::Sentry => "Sentry".to_owned(),
                }
            };
            let (sprite_row, sprite_col) = unit_sprite(unit.kind);
            draw_texture_ex(
                &assets.units,
                x,
                card.y + 8.0,
                WHITE,
                DrawTextureParams {
                    dest_size: Some(vec2(36.0, 27.0)),
                    source: Some(isolated_sprite_source(sprite_row, sprite_col, 64.0, 48.0)),
                    ..Default::default()
                },
            );

            let width = card.w - 20.0;
            let right = x + width;
            let identity_x = x + 41.0;
            let muted = Color::from_rgba(181, 190, 174, 255);
            let status_color = Color::from_rgba(188, 219, 175, 255);
            let state = if unit.kind.is_aircraft() {
                format!("{state} · Fuel {}/{}", unit.fuel, unit.kind.fuel_capacity())
            } else {
                state
            };
            let state = if unit.veteran > 0 {
                format!("{state} · {}", unit.rank_name())
            } else {
                state
            };
            let state_width = measure_text(&state, None, 12, 1.0).width;
            let name_width = measure_text(unit.kind.name(), None, 14, 1.0).width;
            let inline_status = identity_x + name_width + state_width + 8.0 <= right;
            let title_y = card.y + 20.0;
            text_in(
                unit.kind.name(),
                identity_x,
                title_y,
                14.0,
                right - identity_x,
                Color::from_rgba(244, 221, 147, 255),
            );
            text_in(
                &format!(
                    "{} · {}",
                    self.sim.players()[unit.owner].name,
                    self.sim.tile(unit.x, unit.y).terrain.name(),
                ),
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
            let moves = self.sim.unit_move_allowance(unit.owner, unit.kind);
            let metrics = [
                ("Health", format!("{}/{}", unit.hp, stats.hp)),
                ("Moves", format!("{}/{moves}", unit.moves_left)),
                ("Attack", stats.attack.to_string()),
                ("Defense", stats.defense.to_string()),
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
        } else if let Some(index) = self.selected_city_index() {
            let city = &self.sim.cities()[index];
            let forecast = self.sim.city_forecast(city);
            let (food, shields, trade) = (forecast.food, forecast.shields, forecast.trade);
            let foreign = city.size - city.domestic_citizens();
            let cost = city.production.cost();
            context_text(
                &city.name,
                x,
                y + 23.0,
                25.0,
                Color::from_rgba(244, 221, 147, 255),
            );
            context_text(
                &format!(
                    "City size {} ({foreign} foreign)  Plague {:.1}%",
                    city.size,
                    f64::from(self.sim.city_illness(city, shields)) / 10.0
                ),
                x,
                y + 48.0,
                18.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!("Food {}  Shields {}  Trade {}", food, shields, trade),
                x,
                y + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            context_text(
                &format!(
                    "Food store {}/{}",
                    city.food,
                    Simulation::growth_box(city.size)
                ),
                x,
                y + 102.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            context_text(
                &format!("Producing: {}", city.production.name()),
                x,
                y + 132.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            context_text(
                &format!("Production {}/{}", city.shields, cost),
                x,
                y + 156.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
        }
    }
}
