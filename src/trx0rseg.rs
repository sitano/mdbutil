/// Undo log segment slot in a rollback segment header
use std::collections::HashMap;
use std::fmt::Debug;

use crate::fsp0types;
use crate::fut0lst;
use crate::mach;
use crate::trx0sys::mysql_log_t;
use crate::wsrep;

/// Number of undo log slots in a rollback segment file copy
#[allow(non_snake_case)]
pub fn TRX_RSEG_N_SLOTS(page_size: usize) -> u32 {
    (page_size / 16) as u32
}

/// Maximum number of transactions supported by a single rollback segment
#[allow(non_snake_case)]
pub fn TRX_RSEG_MAX_N_TRXS(page_size: usize) -> u32 {
    TRX_RSEG_N_SLOTS(page_size) / 2
}

/// Page number of the header page of an undo log segment
pub const TRX_RSEG_SLOT_PAGE_NO: u32 = 0;

/// Slot size
pub const TRX_RSEG_SLOT_SIZE: u32 = 4;

/// The offset of the rollback segment header on its page
pub const TRX_RSEG: u32 = fsp0types::FSEG_PAGE_DATA;

// Transaction rollback segment header

/// 0xfffffffe = pre-MariaDB 10.3.5 format; 0=MariaDB 10.3.5 or later
pub const TRX_RSEG_FORMAT: u32 = 0;

/// Number of pages in the TRX_RSEG_HISTORY list
pub const TRX_RSEG_HISTORY_SIZE: u32 = 4;

/// Committed transaction logs that have not been purged yet
pub const TRX_RSEG_HISTORY: u32 = 8;

/// Header for the file segment where this page is placed
pub const TRX_RSEG_FSEG_HEADER: u32 = 8 + fut0lst::FLST_BASE_NODE_SIZE;

/// Undo log segment slots
pub const TRX_RSEG_UNDO_SLOTS: u32 =
    8 + fut0lst::FLST_BASE_NODE_SIZE + fsp0types::FSEG_HEADER_SIZE as u32;

/// Maximum transaction ID (valid only if TRX_RSEG_FORMAT is 0)
#[allow(non_snake_case)]
pub fn TRX_RSEG_MAX_TRX_ID(page_size: usize) -> u32 {
    TRX_RSEG_UNDO_SLOTS + TRX_RSEG_N_SLOTS(page_size) * TRX_RSEG_SLOT_SIZE
}

/// 8 bytes offset within the binlog file.
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_BINLOG_OFFSET: u32 = 8;

/// MySQL log file name, 512 bytes, including terminating NUL
/// (valid only if TRX_RSEG_FORMAT is 0).
/// If no binlog information is present, the first byte is NUL.
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_BINLOG_NAME_OFFSET: u32 = 16;

/// Maximum length of binlog file name, including terminating NUL, in bytes
pub const TRX_RSEG_BINLOG_NAME_LEN: u32 = 512;

/// The offset to WSREP XID headers, after TRX_RSEG
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_WSREP_XID_INFO: u32 = 16 + 512;

pub const TRX_RSEG_WSREP_XID_LEN: u32 = TRX_RSEG_WSREP_XID_DATA + wsrep::XIDDATASIZE;

/// WSREP XID format (1 if present and valid, 0 if not present)
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_WSREP_XID_FORMAT: u32 = TRX_RSEG_WSREP_XID_INFO;

/// WSREP XID GTRID length
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_WSREP_XID_GTRID_LEN: u32 = TRX_RSEG_WSREP_XID_INFO + 4;

/// WSREP XID bqual length
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_WSREP_XID_BQUAL_LEN: u32 = TRX_RSEG_WSREP_XID_INFO + 8;

/// WSREP XID data (XIDDATASIZE bytes)
/// Offset after TRX_RSEG_MAX_TRX_ID.
pub const TRX_RSEG_WSREP_XID_DATA: u32 = TRX_RSEG_WSREP_XID_INFO + 12;

#[allow(non_camel_case_types)]
pub struct trx_rseg_t {
    pub format: u32,
    pub history_size: u32,
    pub history: fut0lst::flst_base_node_t,
    pub fseg_header: fsp0types::fseg_header_t,
    pub undo_slots: HashMap<u32, u32>, // slot number -> page number
    pub max_trx_id: u64,
    pub mysql_log: Option<mysql_log_t>,
    pub wsrep_xid: Option<wsrep::wsrep_xid_t>,
}

impl trx_rseg_t {
    /// Reads a trx_rseg_t structure from the given page buffer.
    pub fn from_page(buf: &[u8]) -> trx_rseg_t {
        trx_rseg_t::from_buf(&buf[TRX_RSEG as usize..], buf.len())
    }

    /// Reads a trx_rseg_t structure from the given buffer.
    /// The buffer must be at least `TRX_RSEG_MAX_TRX_ID + 16 + 512 + TRX_RSEG_WSREP_XID_LEN` bytes long.
    pub fn from_buf(buf: &[u8], page_size: usize) -> trx_rseg_t {
        assert!(
            buf.len()
                >= (TRX_RSEG_MAX_TRX_ID(page_size)
                    + TRX_RSEG_WSREP_XID_INFO
                    + TRX_RSEG_WSREP_XID_LEN) as usize
        );

        let format = mach::mach_read_from_4(&buf[TRX_RSEG_FORMAT as usize..]); // 0
        let history_size = mach::mach_read_from_4(&buf[TRX_RSEG_HISTORY_SIZE as usize..]); // 4
        let history = fut0lst::flst_base_node_t::from_buf(&buf[TRX_RSEG_HISTORY as usize..]); // 8
        let fseg_header = fsp0types::fseg_header_t::from_buf(&buf[TRX_RSEG_FSEG_HEADER as usize..]); // 8+16

        let mut undo_slots = HashMap::new();
        for i in 0..TRX_RSEG_N_SLOTS(page_size) {
            let slot_offset = (TRX_RSEG_UNDO_SLOTS + i * TRX_RSEG_SLOT_SIZE) as usize;
            let page_no = mach::mach_read_from_4(&buf[slot_offset..]);
            if page_no != 0xFFFFFFFF {
                undo_slots.insert(i, page_no);
            }
        }

        let max_trx_id_offset = TRX_RSEG_MAX_TRX_ID(page_size) as usize;
        let max_trx_id = mach::mach_read_from_8(&buf[max_trx_id_offset..]);

        let mysql_log = mysql_log_t_from_trx_rseg_buf(&buf[max_trx_id_offset..]);
        let wsrep_xid = wsrep_xid_t_from_trx_rseg_buf(
            &buf[max_trx_id_offset + TRX_RSEG_WSREP_XID_INFO as usize..],
        );

        trx_rseg_t {
            format,
            history_size,
            history,
            fseg_header,
            undo_slots,
            max_trx_id,
            mysql_log,
            wsrep_xid,
        }
    }
}

pub fn mysql_log_t_from_trx_rseg_buf(buf: &[u8]) -> Option<mysql_log_t> {
    assert!(buf.len() >= (TRX_RSEG_BINLOG_NAME_OFFSET + TRX_RSEG_BINLOG_NAME_LEN) as usize);

    let name_bytes = &buf[TRX_RSEG_BINLOG_NAME_OFFSET as usize
        ..(TRX_RSEG_BINLOG_NAME_OFFSET + TRX_RSEG_BINLOG_NAME_LEN) as usize];
    if name_bytes[0] == 0 {
        return None;
    }

    let log_name = match name_bytes.iter().position(|&b| b == 0) {
        Some(pos) => String::from_utf8_lossy(&name_bytes[..pos]).to_string(),
        None => String::from_utf8_lossy(name_bytes).to_string(),
    };

    let log_offset = mach::mach_read_from_8(&buf[TRX_RSEG_BINLOG_OFFSET as usize..]);

    Some(mysql_log_t {
        log_name,
        log_offset,
    })
}

pub fn wsrep_xid_t_from_trx_rseg_buf(buf: &[u8]) -> Option<wsrep::wsrep_xid_t> {
    assert!(buf.len() >= TRX_RSEG_WSREP_XID_LEN as usize);

    let format = mach::mach_read_from_4(&buf[TRX_RSEG_WSREP_XID_FORMAT as usize..]);

    assert!(
        format == 0 || format == 1,
        "Invalid wsrep_xid_t format: {}",
        format
    );

    if format == 0 {
        return None;
    }

    let gtrid_len = mach::mach_read_from_4(&buf[TRX_RSEG_WSREP_XID_GTRID_LEN as usize..]);
    let bqual_len = mach::mach_read_from_4(&buf[TRX_RSEG_WSREP_XID_BQUAL_LEN as usize..]);

    let mut xid_data = [0u8; wsrep::XIDDATASIZE as usize];
    xid_data.copy_from_slice(
        &buf[TRX_RSEG_WSREP_XID_DATA as usize
            ..(TRX_RSEG_WSREP_XID_DATA + wsrep::XIDDATASIZE) as usize],
    );

    Some(wsrep::wsrep_xid_t {
        format,
        gtrid_len,
        bqual_len,
        xid_data,
    })
}

impl Debug for trx_rseg_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slots = self
            .undo_slots
            .iter()
            .filter(|(_slot, page)| **page < 0xFFFFFFFF)
            .map(|(s, p)| UndoSlotPrinter(*s, *p))
            .collect::<Vec<_>>();

        let mut s = f.debug_struct("trx_rseg_t");
        s.field("format", &self.format);
        s.field("history_size", &self.history_size);
        s.field("history", &self.history);
        s.field("fseg_header", &self.fseg_header);
        s.field("undo_slots", &slots);
        s.field("max_trx_id", &self.max_trx_id);
        s.field("mysql_log", &self.mysql_log);
        s.field("wsrep_xid", &self.wsrep_xid);
        s.finish()
    }
}

pub struct UndoSlotPrinter(u32, u32); // slot number, page number

impl Debug for UndoSlotPrinter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} -> {})", self.0, self.1)
    }
}
