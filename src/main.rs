use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;
use mdbutil::mach;

fn main() {
    let config = Config::parse();
    let _log = log::Redo::open(&config).expect("Failed to open redo log");

    let buf: [u8; 4] = [0x00, 0x00, 0x00, 0x01];
    assert_eq!(mach::mach_read_from_4(&buf), 1);

    println!("{}", log::FIRST_LSN);
}
