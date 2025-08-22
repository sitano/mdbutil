use std::{
    io::{Seek, Write},
    path::Path,
};

use bolero::check;
use mdbutil::{
    Lsn,
    log::{CHECKPOINT_1, CHECKPOINT_2, FIRST_LSN, Redo, RedoHeader},
    mtr::Mtr,
    mtr0types::MtrOperation,
};

fn main() {
    let size = 1024 * 1024; // 1 MiB of storage

    let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    check!().with_type().for_each(|lsn: &Lsn| {
        let res = make_redo_log_file(path, size, *lsn);

        if res.is_err() && (*lsn < FIRST_LSN || *lsn >= Lsn::MAX - 16) {
            // Skip invalid LSNs or errors in file creation.
            return;
        }
        res.expect("Failed to create redo log file");

        parse_redo_log_file(path, *lsn).expect("Failed to parse redo log file");
    });
}

fn make_redo_log_file(path: &Path, size: u64, lsn: Lsn) -> std::io::Result<()> {
    let first_lsn = FIRST_LSN;
    let capacity = size - first_lsn;

    let mut log = Redo::writer(path, first_lsn as usize, size).map_err(std::io::Error::other)?;
    let mut writer = log.writer();

    let header = RedoHeader::build_unencrypted_header_10_8(first_lsn, "test_creator")?;
    writer.seek(std::io::SeekFrom::Start(0))?;
    writer.write_all(&header)?;

    let checkpoint = RedoHeader::build_unencrypted_header_10_8_checkpoint(lsn, lsn)?;
    writer.seek(std::io::SeekFrom::Start(CHECKPOINT_1 as u64))?;
    writer.write_all(&checkpoint)?;

    writer.seek(std::io::SeekFrom::Start(CHECKPOINT_2 as u64))?;
    writer.write_all(&checkpoint)?;

    let mut file_checkpoint = vec![];
    Mtr::build_file_checkpoint(&mut file_checkpoint, first_lsn, capacity, lsn)?;
    file_checkpoint.push(0x0); // end marker

    writer.seek(std::io::SeekFrom::Start(lsn))?;
    writer.write_all(&file_checkpoint)?;

    Ok(())
}

fn parse_redo_log_file(path: &Path, lsn: Lsn) -> anyhow::Result<()> {
    let log = Redo::open(path)?;

    assert_eq!(log.header().first_lsn, FIRST_LSN);
    assert!(!log.checkpoint().encrypted);
    assert_eq!(log.checkpoint().checkpoint_lsn, Some(lsn));
    assert_eq!(log.checkpoint().end_lsn, lsn);

    let mut file_checkpoint_lsn = None;
    let mut reader = log.reader();
    let mut mtrs = 0usize;

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

                panic!("Failed to parse MTR: {err:#?}");
            }
        };

        mtrs += chain.mtr.len();

        for mtr in chain.mtr {
            if mtr.op == MtrOperation::FileCheckpoint {
                file_checkpoint_lsn = mtr.file_checkpoint_lsn;
            }
        }
    }

    if mtrs != 1 {
        let filename = path.file_name().unwrap().to_string_lossy();
        std::fs::copy(path, filename.to_string()).expect("Failed to copy redo log file");
    }

    assert!(
        mtrs == 1,
        "Expected 1 MTR, found {mtrs} at checkpoint pos {lsn} (see {filename})",
        filename = path.file_name().unwrap().to_string_lossy()
    );

    assert_eq!(
        file_checkpoint_lsn,
        Some(lsn),
        "Expected file checkpoint LSN to be {lsn}, found {file_checkpoint_lsn:?} (see {path:?})"
    );

    Ok(())
}
