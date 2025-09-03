use std::fmt::Debug;

use crate::{fsp0types, fut0lst, univ, wsrep};

// Transaction undo log
// -------------------------------------------------------------

/// The offset of the undo log page header on pages of the undo log
pub const TRX_UNDO_PAGE_HDR: u32 = fsp0types::FSEG_PAGE_DATA;

// Transaction undo log page header

/// unused; 0 (before MariaDB 10.3.1: 1=TRX_UNDO_INSERT or 2=TRX_UNDO_UPDATE).
pub const TRX_UNDO_PAGE_TYPE: u32 = 0;
/// Byte offset where the undo log records for the LATEST transaction start on this page.
pub const TRX_UNDO_PAGE_START: u32 = 2;
/// Byte offset of the first free byte on the page.
pub const TRX_UNDO_PAGE_FREE: u32 = 4;
/// The file list node in the chain of undo log pages.
pub const TRX_UNDO_PAGE_NODE: u32 = 6;

/// Size of the transaction undo log page header, in bytes.
pub const TRX_UNDO_PAGE_HDR_SIZE: u32 = 6 + fut0lst::FLST_NODE_SIZE;

/// An update undo segment with just one page can be reused if it has at most this many bytes used.
/// We must leave space at least for one new undo log header on the page.
#[allow(non_snake_case)]
pub fn TRX_UNDO_PAGE_REUSE_LIMIT(page_size: u32) -> u32 {
    3 << (univ::page_size_shift(page_size) - 2)
}

#[allow(non_camel_case_types)]
#[derive(Debug)]
pub struct trx_undo_page_t {
    /// unused; 0 (before MariaDB 10.3.1: 1=TRX_UNDO_INSERT or 2=TRX_UNDO_UPDATE).
    pub page_type: u16,
    /// Byte offset where the undo log records for the LATEST transaction start on this page.
    pub start: u16,
    /// Byte offset of the first free byte on the page.
    pub free: u16,
    /// The file list node in the chain of undo log pages.
    pub node: fut0lst::flst_node_t,
}

impl trx_undo_page_t {
    pub fn from_page(page: &[u8]) -> trx_undo_page_t {
        assert!(page.len() >= TRX_UNDO_PAGE_HDR as usize + TRX_UNDO_PAGE_HDR_SIZE as usize);
        trx_undo_page_t::from_buf(&page[TRX_UNDO_PAGE_HDR as usize..])
    }

    /// Reads a transaction undo log page header from the given buffer.
    /// The buffer must be at least `TRX_UNDO_PAGE_HDR_SIZE` bytes long.
    pub fn from_buf(buf: &[u8]) -> trx_undo_page_t {
        assert!(buf.len() >= TRX_UNDO_PAGE_HDR_SIZE as usize);

        let page_type = crate::mach::mach_read_from_2(&buf[TRX_UNDO_PAGE_TYPE as usize..]);
        let start = crate::mach::mach_read_from_2(&buf[TRX_UNDO_PAGE_START as usize..]);
        let free = crate::mach::mach_read_from_2(&buf[TRX_UNDO_PAGE_FREE as usize..]);
        let node = fut0lst::flst_node_t::from_buf(&buf[TRX_UNDO_PAGE_NODE as usize..]);

        trx_undo_page_t {
            page_type,
            start,
            free,
            node,
        }
    }
}

// An update undo log segment may contain several undo logs on its first page if the undo logs took
// so little space that the segment could be cached and reused. All the undo log headers are then
// on the first page, and the last one owns the undo log records on subsequent pages if the segment
// is bigger than one page. If an undo log is stored in a segment, then on the first page it is
// allowed to have zero undo records, but if the segment extends to several pages, then all the
// rest of the pages must contain at least one undo log record.

/// The offset of the undo log segment header on the first page of the undo log segment
pub const TRX_UNDO_SEG_HDR: u32 = TRX_UNDO_PAGE_HDR + TRX_UNDO_PAGE_HDR_SIZE;

// Undo log segment header
// -------------------------------------------------------------

/// TRX_UNDO_ACTIVE, ...
pub const TRX_UNDO_STATE: u32 = 0;

/// Offset of the last undo log header on the segment header page, 0 if none
pub const TRX_UNDO_LAST_LOG: u32 = 2;
/// Header for the file segment which the undo log segment occupies
pub const TRX_UNDO_FSEG_HEADER: u32 = 4;
/// Base node for the list of pages in the undo log segment
pub const TRX_UNDO_PAGE_LIST: u32 = 4 + fsp0types::FSEG_HEADER_SIZE as u32;

/// Size of the undo log segment header
pub const TRX_UNDO_SEG_HDR_SIZE: u32 =
    4 + fsp0types::FSEG_HEADER_SIZE as u32 + fut0lst::FLST_BASE_NODE_SIZE;

// The undo log header. There can be several undo log headers on the first page of an update undo
// log segment.

pub const TRX_UNDO_TRX_ID: u32 = 0; // Transaction start identifier, or 0 if the undo log segment has been completely purged
pub const TRX_UNDO_TRX_NO: u32 = 8; // Transaction end identifier (if the log is in a history list), or 0 if not committed
pub const TRX_UNDO_NEEDS_PURGE: u32 = 16; // (removed in MariaDB 11.0)
pub const TRX_UNDO_LOG_START: u32 = 18; // Offset of the first undo log record of this log on the header page
pub const TRX_UNDO_XID_EXISTS: u32 = 20; // TRUE if undo log header includes X/Open XA transaction identification XID
pub const TRX_UNDO_DICT_TRANS: u32 = 21; // TRUE if the transaction is a table create, index create, or drop transaction
pub const TRX_UNDO_TABLE_ID: u32 = 22; // Id of the table if the preceding field is TRUE
pub const TRX_UNDO_NEXT_LOG: u32 = 30; // Offset of the next undo log header on this page, 0 if none
pub const TRX_UNDO_PREV_LOG: u32 = 32; // Offset of the previous undo log header on this page, 0 if none
pub const TRX_UNDO_HISTORY_NODE: u32 = 34; // If the log is put to the history list, the file list node is here

/// Size of the undo log header without XID information
pub const TRX_UNDO_LOG_OLD_HDR_SIZE: u32 = 34 + fut0lst::FLST_NODE_SIZE;

/// X/Open XA Transaction Identification (XID)
pub const TRX_UNDO_XA_FORMAT: u32 = TRX_UNDO_LOG_OLD_HDR_SIZE; // xid_t::formatID
pub const TRX_UNDO_XA_TRID_LEN: u32 = TRX_UNDO_XA_FORMAT + 4; // xid_t::gtrid_length
pub const TRX_UNDO_XA_BQUAL_LEN: u32 = TRX_UNDO_XA_TRID_LEN + 4; // xid_t::bqual_length
pub const TRX_UNDO_XA_XID: u32 = TRX_UNDO_XA_BQUAL_LEN + 4; // Distributed transaction identifier data

/// Total size of the undo log header with the XA XID
pub const TRX_UNDO_LOG_XA_HDR_SIZE: u32 = TRX_UNDO_XA_XID + wsrep::XIDDATASIZE;
