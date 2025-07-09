use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");

    println!(
        "Log file opened successfully: {}",
        log.buf()[0..32]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
}
