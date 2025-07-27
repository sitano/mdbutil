use std::io::{Seek, Write};

use clap::Parser;
use mdbutil::{
    Lsn, config::Config, log, mtr::Mtr, mtr::MtrChain, mtr0types::MtrOperation, ring::RingReader,
};

fn main() {
    let config = Config::parse();
    let log_file_path = config.get_log_file_path();
    let log = log::Redo::open(&log_file_path).expect("Failed to open redo log");

    println!("Header block: {}", log.header().first_lsn);
    println!("Size: {}, Capacity: {}", log.size(), log.capacity());

    println!("{:#?}", log.header());
    println!("{:#?}", log.checkpoint());

    let mut file_checkpoins_chain = None;
    let mut file_checkpoint_lsn = None;
    let mut reader = log.reader();
    loop {
        let chain = match reader.parse_next() {
            Ok(chain) => chain,
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

        println!(
            "MTR Chain count={}, len={}, lsn={}",
            chain.mtr.len(),
            chain.len,
            chain.lsn
        );

        let mut i = 0;
        for mtr in &chain.mtr {
            if mtr.op == MtrOperation::FileCheckpoint {
                file_checkpoins_chain = Some(chain.clone());
                file_checkpoint_lsn = mtr.file_checkpoint_lsn;
            }

            i += 1;
            println!("  {i}: {mtr}");
        }
    }

    println!("Checkpoint LSN/1: {:?}", log.checkpoint().checkpoints[0]);
    println!("Checkpoint LSN/2: {:?}", log.checkpoint().checkpoints[1]);

    if let Some(file_checkpoint_lsn) = file_checkpoint_lsn {
        println!("File checkpoint chain: {file_checkpoins_chain:?}");
        println!("File checkpoint LSN: {file_checkpoint_lsn}");
    } else {
        eprintln!("WARNING: No file checkpoint found in redo log.");
    }

    if log.header().version != log::FORMAT_10_8 {
        eprintln!("WARNING: the redo log is not in 10.8 format.");
    }

    if log.checkpoint().checkpoint_lsn != Some(log.checkpoint().end_lsn) {
        eprintln!("WARNING: checkpoint LSN is not at the end of the log.");
    }

    if config.write {
        if log.header().version != log::FORMAT_10_8 {
            eprintln!("This tool only supports 10.8 redo logs.");
            return;
        }

        if log.checkpoint().checkpoint_lsn != Some(log.checkpoint().end_lsn) {
            eprintln!("This tool only supports redo logs with a checkpoint at the end.");
            eprintln!("Ensure --innodb_fast_shutdown=0 is set when starting the server.");
            return;
        }

        if file_checkpoint_lsn.is_none() {
            eprintln!("No file checkpoint found in redo log, nothing to write.");
            return;
        }

        let file_checkpoint_lsn = file_checkpoint_lsn.expect("lsn exists") as Lsn;

        let dst = config.srv_log_group_home_dir.join("ib_logfile0.new");

        let mut file_checkpoint = vec![];
        let header = log.header().first_lsn;
        let capacity = log.capacity();
        Mtr::build_file_checkpoint(&mut file_checkpoint, header, capacity, file_checkpoint_lsn)
            .unwrap();
        file_checkpoint.push(0x0); // end marker

        let mut r0 = RingReader::new(file_checkpoint.as_slice());
        let chain = MtrChain::parse_next(&mut r0).unwrap();
        let mtr = chain.mtr[0];

        let target_lsn = log
            .checkpoint()
            .checkpoint_lsn
            .expect("checkpoint lsn must be present");
        let src_reader = log.reader();
        let target_offset = src_reader.reader().pos_to_offset(target_lsn as usize) as u64;

        let target_header = log::RedoHeader::build_unencrypted_header_10_8(
            log.header().first_lsn,
            &log.header().creator,
        )
        .expect("Failed to build header");
        let target_cp_lsn =
            log::RedoHeader::build_unencrypted_header_10_8_checkpoint(target_lsn, target_lsn)
                .expect("Failed to build checkpoint header");

        println!("New MTR: {mtr:#?}");
        println!(
            "Writing file checkpoint: {file_checkpoint:#x?} at pos: {target_offset} \
             ({target_offset:#x})"
        );

        let mut file_writer = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&dst)
            .expect("Failed to open log file for writing");

        // set file size.
        file_writer
            .set_len(log.size())
            .expect("Failed to set file size");

        // header
        file_writer
            .seek(std::io::SeekFrom::Start(0))
            .expect("Failed to seek to end of file");
        file_writer
            .write_all(&target_header)
            .expect("Failed to write file checkpoint");

        // checkpoint lsns
        file_writer
            .seek(std::io::SeekFrom::Start(log::CHECKPOINT_1 as u64))
            .expect("Failed to seek to end of file");
        file_writer
            .write_all(&target_cp_lsn)
            .expect("Failed to write file checkpoint");
        file_writer
            .seek(std::io::SeekFrom::Start(log::CHECKPOINT_2 as u64))
            .expect("Failed to seek to end of file");
        file_writer
            .write_all(&target_cp_lsn)
            .expect("Failed to write file checkpoint");

        // file_checkpoint
        file_writer
            .seek(std::io::SeekFrom::Start(target_offset))
            .expect("Failed to seek to end of file");
        // TODO: wrap
        file_writer
            .write_all(&file_checkpoint)
            .expect("Failed to write file checkpoint");

        file_writer.flush().expect("Failed to flush file writer");
        file_writer.sync_all().expect("Failed to sync file writer");

        drop(file_writer);

        let target_log = log::Redo::open(&dst).expect("Failed to open target redo log");

        println!("Target header block: {}", target_log.header().first_lsn);
        println!(
            "Size: {}, Capacity: {:#x}",
            target_log.size(),
            log.capacity()
        );

        println!("{:#?}", target_log.header());
        println!("{:#?}", target_log.checkpoint());

        let mut file_checkpoint_lsn = None;
        let mut reader = target_log.reader();
        loop {
            let chain = match reader.parse_next() {
                Ok(chain) => chain,
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

            for mtr in chain.mtr {
                if mtr.op == MtrOperation::FileCheckpoint {
                    file_checkpoint_lsn = mtr.file_checkpoint_lsn;
                }

                println!("{mtr:#?}");
            }
        }

        println!(
            "Target checkpoint LSN/1: {:?}",
            target_log.checkpoint().checkpoints[0]
        );
        println!(
            "Target checkpoint LSN/2: {:?}",
            target_log.checkpoint().checkpoints[1]
        );

        let file_checkpoint_lsn =
            file_checkpoint_lsn.expect("No file checkpoint found in redo target_log") as Lsn;
        println!("Target file checkpoint LSN: {file_checkpoint_lsn}");
    }
}
