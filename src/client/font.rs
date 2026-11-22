//! UI typeface. Text is drawn with the bundled Google Sans face instead of
//! Macroquad's built-in font. `CIVCRAFT_FONT=/path/to/font.ttf` overrides it;
//! if neither font loads, drawing falls back to Macroquad's default font.
use macroquad::prelude::*;

static BUNDLED: &[u8] = include_bytes!("../../data/fonts/GoogleSans-Regular.ttf");

thread_local! {
    static FONT: std::cell::OnceCell<Option<Font>> = const { std::cell::OnceCell::new() };
}

fn load() -> Option<Font> {
    if let Some(path) = std::env::var_os("CIVCRAFT_FONT") {
        match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| load_ttf_font_from_bytes(&b).map_err(|e| e.to_string()))
        {
            Ok(font) => return Some(font),
            Err(e) => eprintln!("CIVCRAFT_FONT {path:?} unusable ({e}); using bundled font"),
        }
    }
    match load_ttf_font_from_bytes(BUNDLED) {
        Ok(font) => Some(font),
        Err(e) => {
            eprintln!("bundled UI font failed to load ({e}); using default font");
            None
        }
    }
}

/// Runs `f` with the UI font, loading it on first use. Needs a graphics context.
fn with_font<R>(f: impl FnOnce(Option<&Font>) -> R) -> R {
    FONT.with(|cell| f(cell.get_or_init(load).as_ref()))
}

/// Drop-in replacement for Macroquad's `draw_text` that uses the UI font.
pub(super) fn draw_text(
    text: impl AsRef<str>,
    x: f32,
    y: f32,
    font_size: f32,
    color: Color,
) -> TextDimensions {
    with_font(|font| {
        draw_text_ex(
            text.as_ref(),
            x,
            y,
            TextParams {
                font,
                font_size: font_size as u16,
                color,
                ..Default::default()
            },
        )
    })
}

/// Measures `text` in the UI font at unit scale.
pub(super) fn measure_text_ui(text: &str, size: u16) -> TextDimensions {
    with_font(|font| measure_text(text, font, size, 1.0))
}
