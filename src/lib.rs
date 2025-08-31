pub mod buf0buf;
pub mod config;
pub mod fil0fil;
pub mod fsp0fsp;
pub mod fsp0types;
pub mod fut0lst;
pub mod log;
pub mod mach;
pub mod mtr;
pub mod mtr0log;
pub mod mtr0types;
pub mod page0page;
pub mod page_buf;
pub mod ring;
pub mod tablespace;
pub mod trx0sys;
pub mod univ;
pub mod ut0byte;
pub mod ut0ut;

// Type (lsn_t) used for all log sequence number storage and arithmetics.
pub type Lsn = u64;

pub const LSN_MAX: Lsn = u64::MAX;
