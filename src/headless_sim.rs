//! Headless executable; shared code and tests live in the library.
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    civcraft::run_headless_cli(&args)
}
