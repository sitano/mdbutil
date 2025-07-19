use clap::Parser;

use mdbutil::config::Config;
use mdbutil::log;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");

    println!("Header block: {}", log.header().first_lsn,);
    println!("Size: {}, Capacity: {:#x}", log.size(), log.capacity());

    println!("{:#?}", log.header());
    println!("{:#?}", log.checkpoint());

    let mut reader = log.reader();
    loop {
        let mtr = match reader.parse_next() {
            Ok(mtr) => mtr,
            Err(err) => {
                // test for EOM.
                if let Some(err) = err.downcast_ref::<std::io::Error>()
                    && err.kind() == std::io::ErrorKind::NotFound
                {
                    break;
                }

                eprintln!("\nERROR: {err:?}");
                break;
            }
        };

        println!("{mtr:#?}");
    }
}
