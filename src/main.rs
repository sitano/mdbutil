use std::io::{Seek, Write};

use clap::Parser;

use mdbutil::Lsn;
use mdbutil::config::Config;
use mdbutil::log;
use mdbutil::mtr::Mtr;
use mdbutil::mtr0types::mfile_type_t::FILE_CHECKPOINT;
use mdbutil::ring::RingReader;

fn main() {
    let config = Config::parse();
    let log = log::Redo::open(&config).expect("Failed to open redo log");

    println!("Header block: {}", log.header().first_lsn,);
    println!("Size: {}, Capacity: {:#x}", log.size(), log.capacity());

    println!("{:#?}", log.header());
    println!("{:#?}", log.checkpoint());

    let mut file_checkpoint_lsn = None;
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

        if mtr.op == FILE_CHECKPOINT as u8 {
            file_checkpoint_lsn = mtr.file_checkpoint_lsn;
        }

        println!("{mtr:#?}");
    }

    // TODO: verify file_checkpoint is the last one.

    let file_checkpoint_lsn =
        file_checkpoint_lsn.expect("No file checkpoint found in redo log") as Lsn;
    println!("File checkpoint LSN: {file_checkpoint_lsn}");

    if config.write {
        // copy file
        let src = "./data4/ib_logfile0";
        let dst = "./data4/ib_logfile0.copy";
        if let Err(e) = std::fs::copy(src, dst) {
            eprintln!("Failed to copy file: {e}");
        } else {
            println!("File copied successfully from {src} to {dst}");
        }

        let src_config = Config {
            srv_log_group_home_dir: "./data4".into(),
            write: false,
        };
        let src_log = log::Redo::open(&src_config).expect("Failed to open source redo log");

        // TODO: verify file_checkpoint is correct ending.

        let mut file_checkpoint = vec![];
        Mtr::build_file_checkpoint(&mut file_checkpoint, file_checkpoint_lsn).unwrap();
        file_checkpoint.push(0x0); // end marker

        let mut r0 = RingReader::new(file_checkpoint.as_slice());
        let mtr = Mtr::parse_next(&mut r0).unwrap();

        // TODO: lsn to offset
        let pos = src_log
            .checkpoint()
            .checkpoint_lsn
            .expect("checkpoint lsn must be present");

        println!("New MTR: {mtr:#?}");
        println!("Writing file checkpoint: {file_checkpoint:#x?} at pos: {pos} ({pos:#x})");

        let mut file_writer = std::fs::OpenOptions::new()
            .write(true)
            .open("./data4/ib_logfile0.copy")
            .expect("Failed to open log file for writing");
        file_writer
            .seek(std::io::SeekFrom::Start(pos))
            .expect("Failed to seek to end of file");
        file_writer
            .write_all(&file_checkpoint)
            .expect("Failed to write file checkpoint");
        file_writer.flush().expect("Failed to flush file writer");
        file_writer.sync_all().expect("Failed to sync file writer");
    }
}
