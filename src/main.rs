use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");

    println!("{:#?}", log.header());
    println!("{:#?}", log.checkpoint());
}
