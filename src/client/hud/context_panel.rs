//! Context-sensitive controls for the currently selected city or unit.

use super::*;

/// Space the End Turn button and its bottom margin take at the card's foot.
const END_TURN_RESERVED: f32 = 38.0;
/// Space between the info block's bottom row and the End Turn button.
const INFO_END_TURN_GAP: f32 = 7.0;
/// Where the info block starts on an empty-selection card.
const EMPTY_INFO_TOP: f32 = 10.0;
/// Card height when nothing is selected.
pub(in crate::client) const EMPTY_CARD_H: f32 =
    EMPTY_INFO_TOP + info_block::INFO_GRID_H + INFO_END_TURN_GAP + END_TURN_RESERVED;

/// Offset of the info block below a unit card's top, which depends on whether
/// the status text shares the name line or wraps beneath it.
fn unit_info_top(status_inline: bool) -> f32 {
    if status_inline { 44.0 } else { 56.0 }
}

/// Height of a selected unit's card.
pub(in crate::client) fn unit_card_height(status_inline: bool) -> f32 {
    unit_info_top(status_inline) + info_block::INFO_GRID_H + INFO_END_TURN_GAP + END_TURN_RESERVED
}

/// Whether the unit's status text fits on the same line as its name, given the
/// card width. Shared by drawing and by card sizing so they always agree.
pub(in crate::client) fn unit_status_inline(
    view: &crate::sim::UnitContextView,
    card_width: f32,
) -> bool {
    let name_width = measure_cached(view.kind.name(), 14).width;
    let state_width = measure_cached(&view.status, 12).width;
    41.0 + name_width + state_width + 8.0 <= card_width - 20.0
}

impl Game {
    pub(super) fn draw_context_panel(&self, assets: &Assets, card: Rect, button_y: f32) {
        let x = card.x + 10.0;
        let y = card.y + 4.0;
        let city_offset = INFO_BLOCK_H + 4.0;
        let spacing = ((button_y - y - 6.0) / (156.0 + city_offset)).clamp(0.0, 1.0);
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
            let inline_status = unit_status_inline(&view, card.w);
            let state = if self.selected_units.len() > 1 {
                format!(
                    "{} selected · actions for active",
                    self.selected_units.len()
                )
            } else {
                view.status.clone()
            };
            let state_width = measure_cached(&state, 12).width;
            let inline_status = if self.selected_units.len() > 1 {
                let name_width = measure_cached(view.kind.name(), 14).width;
                identity_x + name_width + state_width + 8.0 <= right
            } else {
                inline_status
            };
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

            self.draw_info_block(card, card.y + unit_info_top(inline_status));
        } else if let Some(city_id) = self.selected_city
            && let Some(city) = self.sim.city_context_view(city_id)
        {
            self.draw_info_block(card, y + 54.0 * spacing);
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
                y + city_offset + 76.0,
                17.0,
                Color::from_rgba(177, 213, 159, 255),
            );
            context_text(
                &format!("Food store {}/{}", city.food_store, city.growth_box),
                x,
                y + city_offset + 102.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
            let culture_line = match city.next_border_threshold {
                Some(threshold) => format!(
                    "Culture {} (+{})  Next border: {}",
                    city.culture, city.culture_per_turn, threshold
                ),
                None => format!(
                    "Culture {} (+{})  Maximum borders",
                    city.culture, city.culture_per_turn
                ),
            };
            context_text(
                &culture_line,
                x,
                y + city_offset + 126.0,
                16.0,
                Color::from_rgba(200, 177, 238, 255),
            );
            context_text(
                &format!("Producing: {}", city.production_name),
                x,
                y + city_offset + 150.0,
                18.0,
                Color::from_rgba(145, 204, 224, 255),
            );
            context_text(
                &format!(
                    "Production {}/{}",
                    city.production_stock, city.production_cost
                ),
                x,
                y + city_offset + 174.0,
                17.0,
                Color::from_rgba(229, 226, 209, 255),
            );
        } else {
            self.draw_info_block(card, card.y + EMPTY_INFO_TOP);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Space between the bottom of the info block and the End Turn button.
    fn gap_above_end_turn(card_height: f32, info_top: f32) -> f32 {
        (card_height - END_TURN_RESERVED) - (info_top + info_block::INFO_GRID_H)
    }

    #[test]
    fn unit_card_leaves_the_same_small_gap_above_end_turn_with_or_without_wrapped_status() {
        for inline in [true, false] {
            let gap = gap_above_end_turn(unit_card_height(inline), unit_info_top(inline));
            assert_eq!(gap, INFO_END_TURN_GAP, "inline status: {inline}");
        }
        assert_eq!(unit_card_height(true), 128.0);
        assert_eq!(unit_card_height(false), 140.0);
    }

    #[test]
    fn empty_card_leaves_the_same_small_gap_above_end_turn() {
        assert_eq!(
            gap_above_end_turn(EMPTY_CARD_H, EMPTY_INFO_TOP),
            INFO_END_TURN_GAP
        );
    }
}
