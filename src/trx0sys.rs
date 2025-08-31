use std::fmt::Debug;

use crate::fsp0types;
use crate::mach;

// The offset of the transaction system header on the page
pub const TRX_SYS: u32 = fsp0types::FSEG_PAGE_DATA;

// Transaction system header
//
// In old versions of InnoDB, this persisted the value of trx_sys.get_max_trx_id().
// Starting with MariaDB 10.3.5, the field TRX_RSEG_MAX_TRX_ID in rollback segment header pages
// and the fields TRX_UNDO_TRX_ID, TRX_UNDO_TRX_NO in undo log pages are used instead.
// The field only exists for the purpose of upgrading from older MySQL or MariaDB versions.
pub const TRX_SYS_TRX_ID_STORE: u32 = 0;
pub const TRX_SYS_FSEG_HEADER: u32 = 8; // segment header for the tablespace segment the trx system is created into
pub const TRX_SYS_RSEGS: u32 = 8 + fsp0types::FSEG_HEADER_SIZE as u32; // start of the array of rollback segment specification slots

// Rollback segment specification slot offsets
//
// the tablespace ID of an undo log header; FIL_NULL if unused
pub const TRX_SYS_RSEG_SPACE: u32 = 0;
// the page number of an undo log header, or FIL_NULL if unused
pub const TRX_SYS_RSEG_PAGE_NO: u32 = 4;
// Size of a rollback segment specification slot
pub const TRX_SYS_RSEG_SLOT_SIZE: u32 = 8;

// Maximum length of MySQL binlog file name, in bytes.
pub const TRX_SYS_MYSQL_LOG_NAME_LEN: usize = 512;
// Contents of TRX_SYS_MYSQL_LOG_MAGIC_N_FLD
pub const TRX_SYS_MYSQL_LOG_MAGIC_N: u32 = 873_422_344;

// The offset of the MySQL binlog offset info in the trx system header
pub const TRX_SYS_MYSQL_LOG_INFO_END: usize = 1000;
pub const TRX_SYS_MYSQL_LOG_MAGIC_N_FLD: usize = 0; // magic number field
pub const TRX_SYS_MYSQL_LOG_OFFSET: usize = 4; // 64-bit offset within that file
pub const TRX_SYS_MYSQL_LOG_NAME: usize = 12; // MySQL log file name

// Memory map TRX_SYS_PAGE_NO = 5 when srv_page_size = 4096
//
// 0...37 FIL_HEADER
// 38...45 TRX_SYS_TRX_ID_STORE
// 46...55 TRX_SYS_FSEG_HEADER (FSEG_HEADER_SIZE == 10)
// 56      TRX_SYS_RSEGS
//   56...59  TRX_SYS_RSEG_SPACE       for slot 0
//   60...63  TRX_SYS_RSEG_PAGE_NO     for slot 0
//   64...67  TRX_SYS_RSEG_SPACE       for slot 1
//   68...71  TRX_SYS_RSEG_PAGE_NO     for slot 1
//   ....
//  594..597  TRX_SYS_RSEG_SPACE       for slot 72
//  598..601  TRX_SYS_RSEG_PAGE_NO     for slot 72
//   ...
//   ...1063  TRX_SYS_RSEG_PAGE_NO     for slot 126
//
// (srv_page_size-3500 WSREP ::: FAIL would overwrite undo tablespace
// space_id, page_no pairs :::)
// 596 TRX_SYS_WSREP_XID_INFO             TRX_SYS_WSREP_XID_MAGIC_N_FLD
// 600 TRX_SYS_WSREP_XID_FORMAT
// 604 TRX_SYS_WSREP_XID_GTRID_LEN
// 608 TRX_SYS_WSREP_XID_BQUAL_LEN
// 612 TRX_SYS_WSREP_XID_DATA   (len = 128)
// 739 TRX_SYS_WSREP_XID_DATA_END
//
// FIXED WSREP XID info offsets for 4k page size 10.0.32-galera
// (srv_page_size-2500)
// 1596 TRX_SYS_WSREP_XID_INFO             TRX_SYS_WSREP_XID_MAGIC_N_FLD
// 1600 TRX_SYS_WSREP_XID_FORMAT
// 1604 TRX_SYS_WSREP_XID_GTRID_LEN
// 1608 TRX_SYS_WSREP_XID_BQUAL_LEN
// 1612 TRX_SYS_WSREP_XID_DATA   (len = 128)
// 1739 TRX_SYS_WSREP_XID_DATA_END
//
// (srv_page_size - 2000 MYSQL MASTER LOG)
// 2096   TRX_SYS_MYSQL_MASTER_LOG_INFO   TRX_SYS_MYSQL_LOG_MAGIC_N_FLD
// 2100   TRX_SYS_MYSQL_LOG_OFFSET_HIGH
// 2104   TRX_SYS_MYSQL_LOG_OFFSET_LOW
// 2108   TRX_SYS_MYSQL_LOG_NAME
//
// (srv_page_size - 1000 MYSQL LOG)
// 3096   TRX_SYS_MYSQL_LOG_INFO          TRX_SYS_MYSQL_LOG_MAGIC_N_FLD
// 3100   TRX_SYS_MYSQL_LOG_OFFSET_HIGH
// 3104   TRX_SYS_MYSQL_LOG_OFFSET_LOW
// 3108   TRX_SYS_MYSQL_LOG_NAME
//
// (srv_page_size - 200 DOUBLEWRITE)
// 3896   TRX_SYS_DOUBLEWRITE		TRX_SYS_DOUBLEWRITE_FSEG
// 3906         TRX_SYS_DOUBLEWRITE_MAGIC
// 3910         TRX_SYS_DOUBLEWRITE_BLOCK1
// 3914         TRX_SYS_DOUBLEWRITE_BLOCK2
// 3918         TRX_SYS_DOUBLEWRITE_REPEAT
// 3930         TRX_SYS_DOUBLEWRITE_SPACE_ID_STORED_N
//
// (srv_page_size - 8, TAILER)
// 4088..4096	FIL_TAILER

#[allow(non_snake_case)]
pub fn TRX_SYS_WSREP_XID_INFO(page_size: usize) -> u32 {
    std::cmp::max(page_size - 3500, 1596) as u32
}

pub const TRX_SYS_WSREP_XID_MAGIC_N_FLD: u32 = 0;
pub const TRX_SYS_WSREP_XID_MAGIC_N: u32 = 0x7773_7265;
// XID field: formatID, gtrid_len, bqual_len, xid_data.
pub const TRX_SYS_WSREP_XID_LEN: u32 = 4 + 4 + 4 + XIDDATASIZE;
pub const TRX_SYS_WSREP_XID_FORMAT: u32 = 4;
pub const TRX_SYS_WSREP_XID_GTRID_LEN: u32 = 8;
pub const TRX_SYS_WSREP_XID_BQUAL_LEN: u32 = 12;
pub const TRX_SYS_WSREP_XID_DATA: u32 = 16;

// Reference: sql/handler.h
pub const XIDDATASIZE: u32 = MYSQL_XIDDATASIZE;
//  struct st_mysql_xid is binary compatible with the XID structure as
//  in the X/Open CAE Specification, Distributed Transaction Processing:
//  The XA Specification, X/Open Company Ltd., 1991.
//  http://www.opengroup.org/bookstore/catalog/c193.htm
//
//  @see XID in sql/handler.h
//
//  Reference: include/mysql/plugin.h
pub const MYSQL_XIDDATASIZE: u32 = 128;

// Doublewrite buffer
//
// The offset of the doublewrite buffer header on the trx system header page */
pub const TRX_SYS_DOUBLEWRITE_END: u32 = 200;

/// Transaction system header structure.
/// This structure is stored in the page TRX_SYS_PAGE_NO of the system tablespace and in the undo
/// tablespaces.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct trx_sys_t {
    pub id_store: u64,
    pub fseg_header: fsp0types::fseg_header_t,
    pub rsegs: Vec<trx_sys_rseg_t>,
    pub wsrep_xid: trx_sys_wsrep_xid_t,
    pub mysql_log: trx_sys_mysql_log_t,
    pub doublewrite: trx_sys_doublewrite_t,
}

/// WSREP XID info structure stored in the trx_sys_t header.
#[allow(non_camel_case_types)]
#[derive(Clone)]
pub struct trx_sys_wsrep_xid_t {
    pub magic: u32,
    pub format: u32,
    pub gtrid_len: u32,
    pub bqual_len: u32,
    pub xid_data: [u8; XIDDATASIZE as usize],
}

/// MariaDB binlog info structure stored in the trx_sys_t header.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct trx_sys_mysql_log_t {
    pub magic: u32,
    pub log_offset: u64,
    pub name: String,
}

/// Doublewrite buffer info structure stored in the trx_sys_t header.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub struct trx_sys_doublewrite_t {
    pub fseg: fsp0types::fseg_header_t,
    pub magic: u32,
    pub block1: u32,
    pub block2: u32,
    pub magic_repeat: u32,
    pub block1_repeat: u32,
    pub block2_repeat: u32,
}

/// Rollback segment specification slot consisting of (space_id, page_no) pointer.
/// The space_id and page_no refer to the header page of a rollback segment (undo tablespace).
/// If space_id == FIL_NULL, the slot is unused.
/// Part of the trx_sys_t structure.
#[allow(non_camel_case_types)]
#[derive(Clone)]
pub struct trx_sys_rseg_t {
    pub space_id: u32,
    pub page_no: u32,
}

impl trx_sys_rseg_t {
    pub fn from_buf(buf: &[u8]) -> Self {
        assert!(buf.len() >= TRX_SYS_RSEG_SLOT_SIZE as usize);

        let space_id = mach::mach_read_from_4(&buf[TRX_SYS_RSEG_SPACE as usize..]);
        let page_no = mach::mach_read_from_4(&buf[TRX_SYS_RSEG_PAGE_NO as usize..]);

        trx_sys_rseg_t { space_id, page_no }
    }
}

impl Debug for trx_sys_rseg_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(space_id: {}, page_no: {})",
            self.space_id, self.page_no
        )
    }
}

impl trx_sys_wsrep_xid_t {
    pub fn from_buf(buf: &[u8]) -> Self {
        assert!(buf.len() >= 4 + TRX_SYS_WSREP_XID_LEN as usize);

        let magic = mach::mach_read_from_4(&buf[TRX_SYS_WSREP_XID_MAGIC_N_FLD as usize..]);
        let format = mach::mach_read_from_4(&buf[TRX_SYS_WSREP_XID_FORMAT as usize..]);
        let gtrid_len = mach::mach_read_from_4(&buf[TRX_SYS_WSREP_XID_GTRID_LEN as usize..]);
        let bqual_len = mach::mach_read_from_4(&buf[TRX_SYS_WSREP_XID_BQUAL_LEN as usize..]);
        let mut xid_data = [0u8; XIDDATASIZE as usize];
        xid_data.copy_from_slice(
            &buf[TRX_SYS_WSREP_XID_DATA as usize..(TRX_SYS_WSREP_XID_DATA + XIDDATASIZE) as usize],
        );

        trx_sys_wsrep_xid_t {
            magic,
            format,
            gtrid_len,
            bqual_len,
            xid_data,
        }
    }
}

impl Debug for trx_sys_wsrep_xid_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("trx_sys_wsrep_xid_t")
            .field("magic", &self.magic)
            .field("format", &self.format)
            .field("gtrid_len", &self.gtrid_len)
            .field("bqual_len", &self.bqual_len)
            .field(
                "xid_data",
                &self
                    .xid_data
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<Vec<String>>()
                    .join(""),
            )
            .finish()
    }
}

impl trx_sys_mysql_log_t {
    pub fn from_buf(buf: &[u8]) -> Self {
        let magic = mach::mach_read_from_4(&buf[TRX_SYS_MYSQL_LOG_MAGIC_N_FLD..]);
        let log_offset = mach::mach_read_from_8(&buf[TRX_SYS_MYSQL_LOG_OFFSET..]);
        let name_bytes =
            &buf[TRX_SYS_MYSQL_LOG_NAME..(TRX_SYS_MYSQL_LOG_NAME + TRX_SYS_MYSQL_LOG_NAME_LEN)];
        let name = String::from_utf8_lossy(
            &name_bytes
                .iter()
                .cloned()
                .take_while(|&b| b != 0)
                .collect::<Vec<u8>>(),
        )
        .to_string();

        trx_sys_mysql_log_t {
            magic,
            log_offset,
            name,
        }
    }
}

impl trx_sys_doublewrite_t {
    pub fn from_buf(buf: &[u8]) -> Self {
        assert!(buf.len() >= 34); // Minimum size for doublewrite_t

        let fseg =
            fsp0types::fseg_header_t::from_buf(&buf[0..fsp0types::FSEG_HEADER_SIZE as usize]);

        let magic = mach::mach_read_from_4(&buf[10..]);
        let block1 = mach::mach_read_from_4(&buf[14..]);
        let block2 = mach::mach_read_from_4(&buf[18..]);

        let magic_repeat = mach::mach_read_from_4(&buf[22..]);
        let block1_repeat = mach::mach_read_from_4(&buf[26..]);
        let block2_repeat = mach::mach_read_from_4(&buf[30..]);

        trx_sys_doublewrite_t {
            fseg,
            magic,
            block1,
            block2,
            magic_repeat,
            block1_repeat,
            block2_repeat,
        }
    }
}

impl trx_sys_t {
    pub fn from_page(page: &[u8]) -> Self {
        Self::from_buf(&page[TRX_SYS as usize..], page.len())
    }

    pub fn from_buf(buf: &[u8], page_size: usize) -> Self {
        let id_store = mach::mach_read_from_8(&buf[TRX_SYS_TRX_ID_STORE as usize..]); // 0
        let fseg_header = fsp0types::fseg_header_t::from_buf(&buf[TRX_SYS_FSEG_HEADER as usize..]); // 8

        let num_slots = 127;
        let mut rsegs: Vec<trx_sys_rseg_t> = Vec::with_capacity(num_slots as usize);

        for i in 0..num_slots {
            let slot_offset = TRX_SYS_RSEGS + i * TRX_SYS_RSEG_SLOT_SIZE; // 18 + i*8
            let slot = trx_sys_rseg_t::from_buf(&buf[slot_offset as usize..]);
            rsegs.push(slot);
        }

        // buf[] starts from TRX_SYS offset, but the struct offset starts from 0.
        let wsrep_xid_buf = &buf[(TRX_SYS_WSREP_XID_INFO(page_size) - TRX_SYS) as usize
            ..(TRX_SYS_WSREP_XID_INFO(page_size) + 4 + TRX_SYS_WSREP_XID_LEN - TRX_SYS) as usize];
        let mysql_log_buf = &buf[page_size - TRX_SYS_MYSQL_LOG_INFO_END - TRX_SYS as usize..];
        let doublewrite_buf = &buf[page_size - (TRX_SYS_DOUBLEWRITE_END + TRX_SYS) as usize..];

        Self {
            id_store,
            fseg_header,
            rsegs,
            wsrep_xid: trx_sys_wsrep_xid_t::from_buf(wsrep_xid_buf),
            mysql_log: trx_sys_mysql_log_t::from_buf(mysql_log_buf),
            doublewrite: trx_sys_doublewrite_t::from_buf(doublewrite_buf),
        }
    }
}
