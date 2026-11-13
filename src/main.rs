//! Desktop executable; shared code and tests live in the library.
use civcraft::desktop_window_conf;

#[macroquad::main(desktop_window_conf)]
async fn main() {
    civcraft::run_desktop().await;
}
