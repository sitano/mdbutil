use std::cmp::min;
use std::io::Write;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use crc32c::crc32c;
use mmap_rs::{Mmap, MmapFlags, MmapOptions};

use crate::Lsn;
use crate::{config::Config, mach, mtr::Mtr, ring::RingReader};

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
pub const FORMAT_ENC_10_2: u32 = FORMAT_10_2 | FORMAT_ENCRYPTED;
/// The MariaDB 10.3.2 log format.
pub const FORMAT_10_3: u32 = 103;
pub const FORMAT_ENC_10_3: u32 = FORMAT_10_3 | FORMAT_ENCRYPTED;
/// The MariaDB 10.4.0 log format.
pub const FORMAT_10_4: u32 = 104;
/// The MariaDB 10.4.0 log format (only with innodb_encrypt_log=ON)
pub const FORMAT_ENC_10_4: u32 = FORMAT_10_4 | FORMAT_ENCRYPTED;
/// Encrypted MariaDB redo log
pub const FORMAT_ENCRYPTED: u32 = 1u32 << 31;
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
    size: u64,
    // The header of the redo log file.
    hdr: RedoHeader,
    // Checkpoint coordinates, if any.
    checkpoint: RedoCheckpointCoordinate,
}

pub struct RedoReader<'a> {
    reader: RingReader<'a>,
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

// Redo log encryption key ID.
pub const LOG_DEFAULT_ENCRYPTION_KEY: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoHeader {
    pub version: u32,
    pub first_lsn: Lsn,
    pub creator: String,
    pub crc: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoCheckpointCoordinate {
    pub checkpoints: [RedoHeaderCheckpoint; 2],
    pub checkpoint_lsn: Option<Lsn>,
    // Position of the checkpoint block entry in the log file.
    // can be CHECKPOINT_1 or CHECKPOINT_2.
    pub checkpoint_no: Option<usize>,
    pub end_lsn: Lsn,
    pub encrypted: bool,
    pub version: u32,
    // Redo log is after a restore operation.
    pub start_after_restore: bool,
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct RedoHeaderCheckpoint {
    pub checkpoint_lsn: Lsn,
    pub end_lsn: Lsn,
    pub checksum: u32,
}

impl Redo {
    pub fn open(log_file_path: &PathBuf) -> anyhow::Result<Redo> {
        let log_file = std::fs::File::open(log_file_path)
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

        // TODO: support Write
        let mmap = unsafe {
            MmapOptions::new(log_size as usize)
                .context("mmap option")?
                .with_file(&log_file, 0u64)
                .with_flags(MmapFlags::SHARED)
                .map()
                .context("mmap log file")?
        };

        let multiple_log_files = Self::search_multiple_log_files(
            log_file_path
                .parent()
                .context("log file parent must exist")?
                .to_path_buf(),
            log_size,
        )
        .context("check multiple log files")?;
        if multiple_log_files > 0 {
            // Multiple ones are possible if we are upgrading from before MariaDB Server 10.5.1.
            // We do not support that.
            return Err(anyhow::anyhow!(
                "multiple redo log files found. upgrading from before MariaDB Server 10.5.1 is not supported"
            ));
        }

        let hdr = Redo::parse_header(mmap.as_slice()).context("parse header")?;
        let checkpoint = Redo::parse_header_checkpoint(mmap.as_slice(), &hdr, multiple_log_files)
            .context("parse redo log checkpoint")?;

        Ok(Redo {
            mmap,
            size: log_size,
            hdr,
            checkpoint,
        })
    }

    pub fn buf(&self) -> &[u8] {
        self.mmap.as_slice()
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn header(&self) -> &RedoHeader {
        &self.hdr
    }

    pub fn checkpoint(&self) -> &RedoCheckpointCoordinate {
        &self.checkpoint
    }

    fn search_multiple_log_files(dir: PathBuf, size: u64) -> anyhow::Result<usize> {
        let mut found = 0;

        for i in 1..101 {
            let log_file_x_path = dir.join(Config::get_log_file_x(i));
            if !log_file_x_path.exists() {
                break;
            }

            found += 1;

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

    pub fn parse_header(buf: &[u8]) -> anyhow::Result<RedoHeader> {
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

        // The original InnoDB redo log format does not have a checksum.
        if version != FORMAT_3_23 {
            let (ok, hdr_crc) = verify_crc_block(&buf[..512], crc);
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

    pub fn parse_header_checkpoint(
        buf: &[u8],
        hdr: &RedoHeader,
        multiple_log_files: usize,
    ) -> anyhow::Result<RedoCheckpointCoordinate> {
        let mut checkpoint = RedoCheckpointCoordinate {
            checkpoints: [
                RedoHeaderCheckpoint::default(),
                RedoHeaderCheckpoint::default(),
            ],
            checkpoint_lsn: None,
            checkpoint_no: None,
            end_lsn: hdr.first_lsn,
            encrypted: false,
            version: hdr.version,
            start_after_restore: false,
        };

        match checkpoint.version {
            FORMAT_10_8 => {
                if multiple_log_files > 0 {
                    bail!("InnoDB: Expecting only ib_logfile0, but multiple log files found");
                }

                let second_hdr_u32 = mach::mach_read_from_4(&buf[LOG_HEADER_FORMAT + 4..]);
                if second_hdr_u32 != 0 || hdr.first_lsn < FIRST_LSN {
                    bail!(
                        "InnoDB: Invalid ib_logfile0 header block; the log was created with {}",
                        hdr.creator
                    );
                }

                let whatever_it_is = mach::mach_read_from_4(&buf[LOG_HEADER_CREATOR_END..]);
                if whatever_it_is == 0 {
                    // all good
                } else if !Redo::parse_crypt_header(&buf[LOG_HEADER_CREATOR_END..])? {
                    bail!(
                        "InnoDB: Reading log encryption info failed; the log was created with {}",
                        hdr.creator
                    );
                } else {
                    checkpoint.version = FORMAT_ENC_10_8;
                    checkpoint.encrypted = true;
                }

                let step = CHECKPOINT_2 - CHECKPOINT_1;
                for pos in (CHECKPOINT_1..=CHECKPOINT_2).step_by(step) {
                    // Checkpoint block is 60 bytes long + 4 bytes for the checksum.
                    // - 8 byte: checkpoint_lsn
                    // - 8 byte: end_lsn
                    // - 44 byte: reserved
                    // - 4 byte: checksum
                    let checkpoint_lsn: Lsn = mach::mach_read_from_8(&buf[pos..]);
                    let end_lsn: Lsn = mach::mach_read_from_8(&buf[pos + 8..]);
                    let reserved = &buf[pos + 16..pos + 60];
                    let checksum = mach::mach_read_from_4(&buf[pos + 60..]);

                    if checkpoint_lsn < hdr.first_lsn
                        || end_lsn < checkpoint_lsn
                        || reserved != [0; 44]
                        || checksum != crc32c(&buf[pos..pos + 60])
                    {
                        writeln!(
                            std::io::stderr(),
                            "InnoDB: Invalid checkpoint at {pos}: checkpoint_lsn={checkpoint_lsn}, end_lsn={end_lsn}, reserved={reserved:?}, checksum={checksum}"
                        )?;
                    }

                    if checkpoint_lsn >= checkpoint.checkpoint_lsn.unwrap_or(0) {
                        checkpoint.checkpoint_lsn = Some(checkpoint_lsn);
                        checkpoint.checkpoint_no = Some(if pos == CHECKPOINT_1 { 1 } else { 0 });
                        checkpoint.end_lsn = end_lsn;
                    }

                    checkpoint.checkpoints[(pos - CHECKPOINT_1) / step] = RedoHeaderCheckpoint {
                        checkpoint_lsn,
                        end_lsn,
                        checksum,
                    };
                }

                if hdr.creator.starts_with("Backup ") {
                    checkpoint.start_after_restore = true;
                }
            }
            FORMAT_10_2 | FORMAT_ENC_10_2 | FORMAT_10_3 | FORMAT_ENC_10_3 | FORMAT_10_4
            | FORMAT_ENC_10_4 | FORMAT_10_5 | FORMAT_ENC_10_5 => {
                if (checkpoint.version == FORMAT_10_5 || checkpoint.version == FORMAT_ENC_10_5)
                    && multiple_log_files > 0
                {
                    bail!("InnoDB: Expecting only ib_logfile0, but multiple log files found");
                }

                let log_size = ((buf.len() - 2048) * multiple_log_files) as Lsn;
                for pos in (512_usize..2048).step_by(1024) {
                    let crc = mach::mach_read_from_4(&buf[pos + LOG_HEADER_CRC..]);
                    let (ok, hdr_crc) = verify_crc_block(&buf[pos..pos + 512], crc);
                    if !ok {
                        writeln!(
                            std::io::stderr(),
                            "InnoDB: Invalid checkpoint checksum at {pos}: expected {crc}, got {hdr_crc}"
                        )?;
                        continue;
                    }

                    // TODO: if (log_sys.is_encrypted() && !log_crypt_read_checkpoint_buf(b))
                    if checkpoint.version & FORMAT_ENCRYPTED != 0 {
                        checkpoint.encrypted = true;
                        todo!("Handle encrypted log header parsing");
                        //  sql_print_error("InnoDB: Reading checkpoint encryption info failed./       continue;
                    }

                    let checkpoint_no = mach::mach_read_from_8(&buf[pos..]) as usize;
                    let checkpoint_lsn: Lsn = mach::mach_read_from_8(&buf[pos + 8..]);
                    let end_lsn: Lsn = mach::mach_read_from_8(&buf[pos + 16..]);

                    writeln!(
                        std::io::stderr(),
                        "InnoDB: checkpoint {checkpoint_no} at LSN {checkpoint_lsn} found",
                    )?;

                    if checkpoint_no >= checkpoint.checkpoint_no.unwrap_or(0)
                        && end_lsn >= 0x80c
                        && (end_lsn & !511) + 512 < log_size
                    {
                        checkpoint.checkpoint_lsn = Some(checkpoint_lsn);
                        checkpoint.checkpoint_no = Some(checkpoint_no);
                        checkpoint.end_lsn = end_lsn; // log_offset
                    }
                }

                if checkpoint.checkpoint_lsn.is_none() {
                    bail!(
                        "InnoDB: No valid checkpoint was found; the log was created with {}",
                        hdr.creator
                    );
                }

                // TODO: if (dberr_t err= recv_log_recover_10_5(lsn_offset)) {}
                todo!("Handle log recovery for <=10.5 formats");
                // TODO: upgrade
            }
            _ => {
                bail!(
                    "InnoDB: Unsupported redo log format version: {}",
                    hdr.version
                );
            }
        }

        if checkpoint.checkpoint_lsn.is_none() {
            bail!(
                "InnoDB: No valid checkpoint was found; the log was created with {}",
                hdr.creator
            );
        }

        Ok(checkpoint)
    }

    // Read the encryption information from a log header buffer.
    // See log_crypt_read_header().
    pub fn parse_crypt_header(hdr: &[u8]) -> anyhow::Result<bool> {
        let encryption_key = mach::mach_read_from_4(hdr);
        if encryption_key != LOG_DEFAULT_ENCRYPTION_KEY {
            // No encryption.
            return Ok(false);
        }

        todo!("Handle log encryption header parsing");
    }

    pub fn reader(&self) -> RedoReader<'_> {
        let lsn = if let Some(lsn) = self.checkpoint.checkpoint_lsn {
            lsn
        } else {
            self.hdr.first_lsn
        };

        RedoReader {
            reader: RingReader::buf_at(
                self.mmap.as_slice(),
                self.hdr.first_lsn as usize,
                lsn as usize,
            ),
        }
    }

    /// returns whether the redo log is in the latest format.
    pub fn is_latest(&self) -> bool {
        is_latest(self.hdr.version)
    }

    /// returns redo log capacity in bytes.
    pub fn capacity(&self) -> Lsn {
        self.size - self.hdr.first_lsn
    }

    /// Determine the sequence bit at a log sequence number.
    /// The sequence bit is used to determine whether the log record
    /// corresponds to the current generation (wrap) of the redo log.
    pub fn get_sequence_bit(&self, lsn: Lsn) -> u8 {
        if ((lsn - self.hdr.first_lsn) / self.capacity()) & 1 == 0 {
            0
        } else {
            1
        }
    }
}

fn is_latest(version: u32) -> bool {
    version & (!FORMAT_ENCRYPTED) == FORMAT_10_8
}

/// verifies block checksum where last 4 bytes is crc32.
fn verify_crc_block(block: &[u8], crc: u32) -> (bool, u32) {
    if block.len() < 4 {
        return (false, 0);
    }

    let new = crc32c(&block[0..block.len() - 4]);

    (new == crc, new)
}

impl<'a> RedoReader<'a> {
    pub fn reader(&self) -> &RingReader<'a> {
        &self.reader
    }

    pub fn parse_next(&mut self) -> anyhow::Result<Mtr> {
        Mtr::parse_next(&mut self.reader).context("Mtr::parse_next")
    }
}

impl RedoHeader {
    pub fn build_unencrypted_header_10_8(
        first_lsn: Lsn,
        creator: &str,
    ) -> std::io::Result<[u8; 512]> {
        let mut buf = [0u8; 512];

        mach::mach_write_to_4(&mut buf[LOG_HEADER_FORMAT..], FORMAT_10_8)?;
        mach::mach_write_to_8(&mut buf[LOG_HEADER_START_LSN..], first_lsn)?;

        let creator_len = min(LOG_HEADER_CREATOR_END - LOG_HEADER_CREATOR, creator.len());
        buf[LOG_HEADER_CREATOR..LOG_HEADER_CREATOR + creator_len]
            .copy_from_slice(&creator.as_bytes()[..creator_len]);

        let crc = crc32c(&buf[..LOG_HEADER_CRC]);
        mach::mach_write_to_4(&mut buf[LOG_HEADER_CRC..], crc)?;

        Ok(buf)
    }

    // Checkpoint block is 60 bytes long + 4 bytes for the checksum.
    // - 8 byte: checkpoint_lsn
    // - 8 byte: end_lsn
    // - 44 byte: reserved
    // - 4 byte: checksum
    pub fn build_unencrypted_header_10_8_checkpoint(
        checkpoint_lsn: Lsn,
        end_lsn: Lsn,
    ) -> std::io::Result<[u8; 64]> {
        let mut buf = [0u8; 64];

        mach::mach_write_to_8(&mut buf[0..], checkpoint_lsn)?;
        mach::mach_write_to_8(&mut buf[8..], end_lsn)?;

        let crc = crc32c(&buf[..60]);
        mach::mach_write_to_4(&mut buf[60..], crc)?;

        Ok(buf)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_build_header_10_8() {
        let mut buf = [0u8; FIRST_LSN as usize];
        let hdr = RedoHeader::build_unencrypted_header_10_8(FIRST_LSN, "test_creator")
            .expect("Failed to build header");
        let cp = RedoHeader::build_unencrypted_header_10_8_checkpoint(FIRST_LSN, FIRST_LSN)
            .expect("Failed to build checkpoint");
        buf[0..hdr.len()].copy_from_slice(&hdr);
        buf[CHECKPOINT_1..CHECKPOINT_1 + cp.len()].copy_from_slice(&cp);
        buf[CHECKPOINT_2..CHECKPOINT_2 + cp.len()].copy_from_slice(&cp);

        let header = Redo::parse_header(&buf).expect("Failed to parse header");
        let _checkpoint =
            Redo::parse_header_checkpoint(&buf, &header, 0).expect("Failed to parse checkpoint");
    }
}
