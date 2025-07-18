use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");

    println!("{:#?}", log.header());
    println!("{:#?}", log.checkpoint());

    let mut reader = log.reader();
    println!("{:#?}", reader.parse_next().expect("reader.parse_next"));

    // good redo log will have no mtrs after file_checkpoint.
    println!("{:#?}", reader.parse_next());
}
