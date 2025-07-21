pub mod config;
pub mod log;
pub mod mach;
pub mod mtr;
pub mod mtr0log;
pub mod mtr0types;
pub mod ring;

// Type (lsn_t) used for all log sequence number storage and arithmetics.
pub type Lsn = u64;

pub const LSN_MAX: Lsn = u64::MAX;
