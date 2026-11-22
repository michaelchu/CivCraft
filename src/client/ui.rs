//! Shared visual primitives. All text bounds use measured pixels, including Unicode names.
use super::*;
pub(super) const UI_GAP: f32 = 8.0;
pub(super) const UI_PAD: f32 = 18.0;

/// Upper bound on cached strings; reaching it simply restarts the cache.
const TEXT_CACHE_LIMIT: usize = 4096;

#[derive(Default)]
struct TextMetricsCache {
    /// Screen size and DPI scale the metrics were measured at.
    surface: (f32, f32, f32),
    by_size: std::collections::HashMap<u16, std::collections::HashMap<String, TextDimensions>>,
    entries: usize,
}

thread_local! {
    static TEXT_METRICS: std::cell::RefCell<TextMetricsCache> = Default::default();
}

/// `measure_text` with the UI font and unit scale, memoized by (text, size).
/// The cache is cleared when the window size or DPI scale changes.
pub(super) fn measure_cached(text: &str, size: u16) -> TextDimensions {
    let surface = (screen_width(), screen_height(), screen_dpi_scale());
    TEXT_METRICS.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.surface != surface || cache.entries >= TEXT_CACHE_LIMIT {
            cache.by_size.clear();
            cache.entries = 0;
            cache.surface = surface;
        }
        if let Some(found) = cache.by_size.get(&size).and_then(|m| m.get(text)) {
            return *found;
        }
        let measured = font::measure_text_ui(text, size);
        cache
            .by_size
            .entry(size)
            .or_default()
            .insert(text.to_owned(), measured);
        cache.entries += 1;
        measured
    })
}

pub(super) fn fit_text(text: &str, size: f32, width: f32) -> String {
    if measure_cached(text, size as u16).width <= width {
        return text.to_owned();
    }
    let chars: Vec<_> = text.chars().collect();
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let candidate = format!("{}...", chars[..mid].iter().collect::<String>());
        if measure_cached(&candidate, size as u16).width <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    if width < measure_cached("...", size as u16).width {
        return String::new();
    }
    format!("{}...", chars[..lo].iter().collect::<String>())
}
pub(super) fn text_in(text: &str, x: f32, y: f32, size: f32, width: f32, color: Color) {
    draw_text(fit_text(text, size, width), x, y, size, color);
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
pub(super) fn panel_close_rect(panel: Rect) -> Rect {
    Rect::new(panel.right() - 45.0, panel.y + 12.0, 32.0, 26.0)
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
    let text_w = measure_cached(&label, 14).width;
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

thread_local! {
    static REVIEW_POINTER: std::cell::Cell<Option<Vec2>> = const { std::cell::Cell::new(None) };
}
pub(super) fn ui_pointer() -> Vec2 {
    REVIEW_POINTER
        .with(|pointer| pointer.get())
        .unwrap_or_else(|| {
            let (x, y) = mouse_position();
            vec2(x, y)
        })
}
pub(crate) fn set_review_pointer(pointer: Option<Vec2>) {
    REVIEW_POINTER.with(|value| value.set(pointer));
}
