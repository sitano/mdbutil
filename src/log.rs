use anyhow::Context;
use anyhow::bail;
use crc32c::crc32c;
use mmap_rs::{Mmap, MmapFlags, MmapOptions};

use crate::{config::Config, mach};

// Type (lsn_t) used for all log sequence number storage and arithmetics.
pub type Lsn = u64;

pub const LSN_MAX: Lsn = u64::MAX;

// According to Linux "man 2 read" and "man 2 write" this applies to
// both 32-bit and 64-bit systems.
//
// On FreeBSD, the limit is close to the Linux one, INT_MAX.
//
// On Microsoft Windows, the limit is UINT_MAX (4 GiB - 1).
//
// On other systems, the limit typically is up to SSIZE_T_MAX.
pub const OS_FILE_REQUEST_SIZE_MAX: usize = 0x7fff_f000;

/// The maximum buf_size
pub const BUF_SIZE_MAX: usize = OS_FILE_REQUEST_SIZE_MAX;

/// The original (not version-tagged) InnoDB redo log format
pub const FORMAT_3_23: u32 = 0;
/// The MySQL 5.7.9/MariaDB 10.2.2 log format
pub const FORMAT_10_2: u32 = 1;
/// The MariaDB 10.3.2 log format.
pub const FORMAT_10_3: u32 = 103;
/// The MariaDB 10.4.0 log format.
pub const FORMAT_10_4: u32 = 104;
/// Encrypted MariaDB redo log
pub const FORMAT_ENCRYPTED: u32 = 1u32 << 31;
/// The MariaDB 10.4.0 log format (only with innodb_encrypt_log=ON)
pub const FORMAT_ENC_10_4: u32 = FORMAT_10_4 | FORMAT_ENCRYPTED;
/// The MariaDB 10.5.1 physical redo log format
pub const FORMAT_10_5: u32 = 0x5048_5953;
/// The MariaDB 10.5.1 physical format (only with innodb_encrypt_log=ON)
pub const FORMAT_ENC_10_5: u32 = FORMAT_10_5 | FORMAT_ENCRYPTED;
/// The MariaDB 10.8.0 variable-block-size redo log format
pub const FORMAT_10_8: u32 = 0x5068_7973;
/// The MariaDB 10.8.0 format with innodb_encrypt_log=ON
pub const FORMAT_ENC_10_8: u32 = FORMAT_10_8 | FORMAT_ENCRYPTED;

/// Location of the first checkpoint block
pub const CHECKPOINT_1: usize = 4096;
/// Location of the second checkpoint block
pub const CHECKPOINT_2: usize = 8192;
/// Start of record payload (0x3000)
pub const START_OFFSET: Lsn = 12288;

/// smallest possible log sequence number in the current format
/// (used to be 2048 before FORMAT_10_8).
pub const FIRST_LSN: Lsn = START_OFFSET;

/// Size of a FILE_CHECKPOINT record, including the trailing byte to
/// terminate the mini-transaction and the CRC-32C.
pub const SIZE_OF_FILE_CHECKPOINT: u64 = 3/*type,page_id*/ + 8/*LSN*/ + 1 + 4;

pub struct Redo {
    mmap: Mmap,
}

// Offsets of a log file header.
//
// Log file header format identifier (32-bit unsigned big-endian integer).
// This used to be called LOG_GROUP_ID and always written as 0,
// because InnoDB never supported more than one copy of the redo log.
pub const LOG_HEADER_FORMAT: usize = 0;
// LSN of the start of data in this log file (with format version 1;
// in format version 0, it was called LOG_FILE_START_LSN and at offset 4).
pub const LOG_HEADER_START_LSN: usize = 8;
// A null-terminated string which will contain either the string 'ibbackup'
// and the creation time if the log file was created by mysqlbackup --restore,
// or the MySQL version that created the redo log file.
pub const LOG_HEADER_CREATOR: usize = 16;
// End of the log file creator field.
pub const LOG_HEADER_CREATOR_END: usize = 48;
// CRC-32C checksum of the log file header.
pub const LOG_HEADER_CRC: usize = 508;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoHeader {
    pub version: u32,
    pub first_lsn: Lsn,
    pub creator: String,
    pub crc: u32,
}

impl Redo {
    pub fn open(config: &Config) -> anyhow::Result<Redo> {
        let log_file_path = config.get_log_file_path();
        let log_file = std::fs::File::open(&log_file_path)
            .with_context(|| format!("open log file at {}", log_file_path.display()))?;
        let log_meta = log_file.metadata().context("get metadata for log a file")?;
        let log_size = log_meta.len();

        if log_size < START_OFFSET + SIZE_OF_FILE_CHECKPOINT {
            return Err(anyhow::anyhow!(
                "log file {} is too small: {} bytes, expected at least {} bytes",
                log_file_path.display(),
                log_size,
                START_OFFSET + SIZE_OF_FILE_CHECKPOINT
            ));
        }

        let mmap = unsafe {
            MmapOptions::new(log_size as usize)
                .context("mmap option")?
                .with_file(&log_file, 0u64)
                .with_flags(MmapFlags::SHARED)
                .map()
                .context("mmap log file")?
        };

        if Self::check_multiple_log_files(config, log_size).context("check_multiple_log_files")? {
            // Multiple ones are possible if we are upgrading from before MariaDB Server 10.5.1.
            // We do not support that.
            return Err(anyhow::anyhow!(
                "multiple redo log files found. upgrading from before MariaDB Server 10.5.1 is not supported"
            ));
        }

        Ok(Redo { mmap })
    }

    pub fn buf(&self) -> &[u8] {
        self.mmap.as_slice()
    }

    fn check_multiple_log_files(config: &Config, size: u64) -> anyhow::Result<bool> {
        let mut found = false;

        for i in 1..101 {
            let log_file_x_path = config.get_log_file_x_path(i);
            if !log_file_x_path.exists() {
                break;
            }

            found = true;

            let file = std::fs::File::open(&log_file_x_path)
                .with_context(|| format!("open log file at {}", log_file_x_path.display()))?;
            let file_meta = file.metadata().context("get metadata for log file")?;
            let file_size = file_meta.len();

            if file_size != size {
                return Err(anyhow::anyhow!(
                    "log file {path} has unexpected size: {file_size} bytes, expected {size} bytes. all log files in a group must have the same size",
                    path = log_file_x_path.display(),
                ));
            }
        }

        Ok(found)
    }

    pub fn parse_header(&self) -> anyhow::Result<RedoHeader> {
        let buf = self.buf();

        if buf.len() < 512 {
            return Err(anyhow::anyhow!(
                "log file is too small to contain a header (< 512)"
            ));
        }

        let version = mach::mach_read_from_4(&buf[LOG_HEADER_FORMAT..]);
        let first_lsn: Lsn = mach::mach_read_from_8(&buf[LOG_HEADER_START_LSN..]);
        let creator = String::from_utf8_lossy(&buf[LOG_HEADER_CREATOR..LOG_HEADER_CREATOR_END])
            .trim_end_matches('\0')
            .to_string();
        let crc = mach::mach_read_from_4(&buf[LOG_HEADER_CRC..]);

        // TODO: verify the version is latest or at least that one that use crc32c checksum.

        {
            let (ok, hdr_crc) = RedoHeader::verify_checksum(buf, crc);
            if !ok {
                bail!("log file header checksum mismatch: expected {crc}, got {hdr_crc}");
            }
        }

        Ok(RedoHeader {
            version,
            first_lsn,
            creator,
            crc,
        })
    }
}

impl RedoHeader {
    pub fn verify_checksum(buf: &[u8], crc: u32) -> (bool, u32) {
        if buf.len() < LOG_HEADER_CRC {
            return (false, 0);
        }

        let new = crc32c(&buf[0..LOG_HEADER_CRC]);

        (new == crc, new)
    }
}
