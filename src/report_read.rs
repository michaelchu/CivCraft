//! Expand simulation report evidence without running a simulation.
fn main() -> Result<(), String> {
    civcraft::run_report_reader_cli(&std::env::args().skip(1).collect::<Vec<_>>())
}
