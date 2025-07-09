pub const LOG_FILE_NAME_PREFIX: &str = "ib_logfile";
pub const LOG_FILE_NAME: &str = "ib_logfile0";

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
