use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");
    let header = log.parse_header().expect("Failed to parse redo log header");
    println!("Redo Log Header: {header:#?}");
}
