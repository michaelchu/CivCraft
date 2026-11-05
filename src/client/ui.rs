//! Shared visual primitives. All text bounds use measured pixels, including Unicode names.
use super::*;
pub(super) const UI_GAP: f32 = 8.0;
pub(super) const UI_PAD: f32 = 18.0;

pub(super) fn fit_text(text: &str, size: f32, width: f32) -> String {
    if measure_text(text, None, size as u16, 1.0).width <= width {
        return text.to_owned();
    }
    let chars: Vec<_> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate = format!("{}...", chars[..mid].iter().collect::<String>());
        if measure_text(&candidate, None, size as u16, 1.0).width <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if width < measure_text("...", None, size as u16, 1.0).width {
        return String::new();
    }
    format!("{}...", chars[..lo].iter().collect::<String>())
}
pub(super) fn text_in(text: &str, x: f32, y: f32, size: f32, width: f32, color: Color) {
    draw_text(&fit_text(text, size, width), x, y, size, color);
}
pub(super) fn panel_frame(panel: Rect) {
    draw_rectangle(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        Color::from_rgba(42, 44, 40, 255),
    );
    draw_rectangle_lines(
        panel.x,
        panel.y,
        panel.w,
        panel.h,
        2.0,
        Color::from_rgba(206, 165, 72, 255),
    );
}
/// Fixed-width columns with consistent gutters; never places a control outside its row.
pub(super) fn grid_columns(width: f32, minimum: f32) -> usize {
    ((width + UI_GAP) / (minimum + UI_GAP)).floor().max(1.0) as usize
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_preserves_minimum_control_width_and_gutters() {
        for available in [140.0, 280.0, 444.0, 660.0, 940.0, 1240.0] {
            let columns = grid_columns(available, 112.0);
            let width = (available - UI_GAP * (columns - 1) as f32) / columns as f32;
            assert!(width >= 112.0);
            assert!(columns as f32 * width + UI_GAP * (columns - 1) as f32 <= available + 0.01);
        }
    }
}

/// Prose stays inside a modal and leaves room for its header controls.
pub(super) fn panel_text(panel: Rect) -> impl Fn(&str, f32, f32, f32, Color) {
    move |text, x, y, size, color| {
        let right = if y < panel.y + 40.0 {
            panel.right() - 108.0
        } else {
            panel.right() - UI_PAD
        };
        text_in(text, x, y, size, right - x, color);
    }
}

pub(super) fn pager(
    panel: Rect,
    page: usize,
    last: usize,
    action: impl Fn(usize) -> UiAction,
) -> Vec<(Rect, UiAction)> {
    let mut actions = Vec::new();
    for (enabled, x, label, target) in [
        (
            page > 0,
            panel.x + UI_PAD,
            "Previous",
            page.saturating_sub(1),
        ),
        (page < last, panel.right() - UI_PAD - 88.0, "Next", page + 1),
    ] {
        let button = Rect::new(x, panel.bottom() - 42.0, 88.0, 28.0);
        draw_ui_button(button, label, enabled, false);
        if enabled {
            actions.push((button, action(target)));
        }
    }
    let label = format!("{} / {}", page + 1, last + 1);
    let text_w = measure_text(&label, None, 14, 1.0).width;
    text_in(
        &label,
        panel.x + (panel.w - text_w) * 0.5,
        panel.bottom() - 23.0,
        14.0,
        text_w,
        LIGHTGRAY,
    );
    actions
}
