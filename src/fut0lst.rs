use crate::fil0fil;

/// The physical size of a list base node in bytes.
pub const FLST_BASE_NODE_SIZE: u32 = 4 + 2 * fil0fil::FIL_ADDR_SIZE;

/// The physical size of a list node in bytes.
pub const FLST_NODE_SIZE: u32 = 2 * fil0fil::FIL_ADDR_SIZE;
