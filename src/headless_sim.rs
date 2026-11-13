//! Headless campaign CLI. Gameplay, analytics, and reporting are independent.
#![allow(dead_code, unused_imports)]
mod analytics;
mod headless;
mod mapgen;
mod random;
mod rules;
mod sim;
mod world;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help") {
        println!("{}", headless::config::HELP);
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-analyzers") {
        println!("{}", analytics::registry::IDS.join("\n"));
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-scenarios") {
        println!("{}", headless::scenario::IDS.join("\n"));
        return Ok(());
    }
    headless::run(headless::config::Config::parse(&args)?)
}
