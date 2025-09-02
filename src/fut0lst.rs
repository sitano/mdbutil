use std::fmt::Debug;

use crate::fil0fil;
use crate::mach;

/// The physical size of a list base node in bytes.
pub const FLST_BASE_NODE_SIZE: u32 = 4 + 2 * fil0fil::FIL_ADDR_SIZE;

/// The physical size of a list node in bytes.
pub const FLST_NODE_SIZE: u32 = 2 * fil0fil::FIL_ADDR_SIZE;

#[allow(non_camel_case_types)]
pub struct flst_base_node_t {
    pub len: u32,
    pub first: fil0fil::fil_addr_t,
    pub last: fil0fil::fil_addr_t,
}

#[allow(non_camel_case_types)]
pub struct flst_node_t {
    pub prev: fil0fil::fil_addr_t,
    pub next: fil0fil::fil_addr_t,
}

impl flst_base_node_t {
    /// Reads a list base node from the given buffer.
    /// The buffer must be at least `FLST_BASE_NODE_SIZE` bytes long.
    pub fn from_buf(buf: &[u8]) -> flst_base_node_t {
        assert!(buf.len() >= FLST_BASE_NODE_SIZE as usize);
        let len = mach::mach_read_from_4(&buf[0..]);
        let first = fil0fil::fil_addr_t::from_buf(&buf[4..]);
        let last = fil0fil::fil_addr_t::from_buf(&buf[4 + fil0fil::FIL_ADDR_SIZE as usize..]);
        flst_base_node_t { len, first, last }
    }
}

impl flst_node_t {
    /// Reads a list node from the given buffer.
    /// The buffer must be at least `FLST_NODE_SIZE` bytes long.
    pub fn from_buf(buf: &[u8]) -> flst_node_t {
        assert!(buf.len() >= FLST_NODE_SIZE as usize);
        let prev = fil0fil::fil_addr_t::from_buf(&buf[0..]);
        let next = fil0fil::fil_addr_t::from_buf(&buf[fil0fil::FIL_ADDR_SIZE as usize..]);
        flst_node_t { prev, next }
    }
}

impl Debug for flst_base_node_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.len == 0 {
            return write!(f, "flst_base_node_t {{ len: 0 }}");
        }

        write!(
            f,
            "flst_base_node_t {{ len: {}, first: {:?}, last: {:?} }}",
            self.len, self.first, self.last
        )
    }
}

impl Debug for flst_node_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "flst_node_t {{ prev: {:?}, next: {:?} }}",
            self.prev, self.next
        )
    }
}
