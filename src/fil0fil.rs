use std::{fmt::Debug, io::Read};

use crate::{fsp0types, mach, univ};

/// Common InnoDB file extensions
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum ib_extention {
    NO_EXT = 0,
    IBD = 1,
    ISL = 2,
    CFG = 3,
}

/** Initial size of a single-table tablespace in pages */
pub const FIL_IBD_FILE_INITIAL_SIZE: u32 = 4;

/** 'null' (undefined) page offset in the context of file spaces */
pub const FIL_NULL: u32 = univ::ULINT32_UNDEFINED;

pub const FIL_ADDR_PAGE: u32 = 0; /* first in address is the page offset */
pub const FIL_ADDR_BYTE: u32 = 4; /* then comes 2-byte byte offset within page*/
pub const FIL_ADDR_SIZE: u32 = 6; /* address size is 6 bytes */

/** File space address */
#[allow(non_camel_case_types)]
pub struct fil_addr_t {
    /** page number within a tablespace */
    pub page: u32,
    /** byte offset within the page */
    pub boffset: u16,
}

impl fil_addr_t {
    /// Create a fil_addr_t from a byte slice.
    /// The slice must be at least FIL_ADDR_SIZE bytes long.
    pub fn from_buf(buf: &[u8]) -> fil_addr_t {
        assert!(buf.len() >= FIL_ADDR_SIZE as usize);
        let page = mach::mach_read_from_4(&buf[0..]);
        let boffset = mach::mach_read_from_2(&buf[4..]);
        fil_addr_t { page, boffset }
    }
}

impl Default for fil_addr_t {
    fn default() -> Self {
        fil_addr_t {
            page: FIL_NULL,
            boffset: 0,
        }
    }
}

impl Read for fil_addr_t {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.len() < FIL_ADDR_SIZE as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Buffer too small, need at least {} bytes", FIL_ADDR_SIZE),
            ));
        }

        mach::mach_write_to_4(&mut buf[FIL_ADDR_PAGE as usize..], self.page)?;
        mach::mach_write_to_2(&mut buf[FIL_ADDR_BYTE as usize..], self.boffset)?;

        Ok(FIL_ADDR_SIZE as usize)
    }
}

impl Debug for fil_addr_t {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.page == FIL_NULL {
            return write!(f, "None");
        }

        write!(
            f,
            "fil_addr_t {{ page: {}, boffset: {} }}",
            self.page, self.boffset
        )
    }
}

/* The byte offsets on a file page for various variables @{ */

/// in < MySQL-4.0.14 space id the page belongs to (== 0) but in later versions the 'new' checksum
/// of the page.
pub const FIL_PAGE_SPACE_OR_CHKSUM: u32 = 0;
/// page offset inside space.
pub const FIL_PAGE_OFFSET: u32 = 4;
/// if there is a 'natural' predecessor of the page, its offset.  Otherwise FIL_NULL. This field is
/// not set on BLOB pages, which are stored as a singly-linked list.  See also FIL_PAGE_NEXT.
pub const FIL_PAGE_PREV: u32 = 8;
/// if there is a 'natural' successor of the page, its offset. Otherwise FIL_NULL. B-tree index
/// pages (FIL_PAGE_TYPE contains FIL_PAGE_INDEX) on the same PAGE_LEVEL are maintained as a doubly
/// linked list via FIL_PAGE_PREV and FIL_PAGE_NEXT in the collation order of the smallest user
/// record on each page.
pub const FIL_PAGE_NEXT: u32 = 12;
/// lsn of the end of the newest modification log record to the page.
pub const FIL_PAGE_LSN: u32 = 16;
/// file page type: FIL_PAGE_INDEX,..., 2 bytes.
///
/// The contents of this field can only be trusted in the following case: if the page is an
/// uncompressed B-tree index page, then it is guaranteed that the value is FIL_PAGE_INDEX. The
/// opposite does not hold.
///
///  In tablespaces created by MySQL/InnoDB 5.1.7 or later, the contents of this field is valid for
///  all uncompressed pages.
pub const FIL_PAGE_TYPE: u32 = 24;

/** For the first page in a system tablespace data file(ibdata*, not *.ibd):
the file has been flushed to disk at least up to this lsn
For other pages of tablespaces not in innodb_checksum_algorithm=full_crc32
format: 32-bit key version used to encrypt the page + 32-bit checksum
or 64 bits of zero if no encryption */
pub const FIL_PAGE_FILE_FLUSH_LSN_OR_KEY_VERSION: u32 = 26;

/** This overloads FIL_PAGE_FILE_FLUSH_LSN for RTREE Split Sequence Number */
pub const FIL_RTREE_SPLIT_SEQ_NUM: u32 = FIL_PAGE_FILE_FLUSH_LSN_OR_KEY_VERSION;

/** Start of the page_compressed content */
pub const FIL_PAGE_COMP_ALGO: u32 = FIL_PAGE_FILE_FLUSH_LSN_OR_KEY_VERSION;

/** starting from 4.1.x this contains the space id of the page */
pub const FIL_PAGE_ARCH_LOG_NO_OR_SPACE_ID: u32 = 34;

pub const FIL_PAGE_SPACE_ID: u32 = FIL_PAGE_ARCH_LOG_NO_OR_SPACE_ID;

pub const FIL_PAGE_DATA: u32 = 38; // start of the data on the page.

/** 32-bit key version used to encrypt the page in full_crc32 format.
For non-encrypted page, it contains 0. */
pub const FIL_PAGE_FCRC32_KEY_VERSION: u32 = 0;

/** page_compressed without innodb_checksum_algorithm=full_crc32 @{ */
/** Number of bytes used to store actual payload data size on
page_compressed pages when not using full_crc32. */
pub const FIL_PAGE_COMP_SIZE: u32 = 0;

/** Number of bytes for FIL_PAGE_COMP_SIZE */
pub const FIL_PAGE_COMP_METADATA_LEN: u32 = 2;

/** Number of bytes used to store actual compression method
for encrypted tables when not using full_crc32. */
pub const FIL_PAGE_ENCRYPT_COMP_ALGO: u32 = 2;

/** Extra header size for encrypted page_compressed pages when
not using full_crc32 */
pub const FIL_PAGE_ENCRYPT_COMP_METADATA_LEN: u32 = 4;
/* @} */

/* File page trailer @{ */

/// the low 4 bytes of this are used
/// to store the page checksum, the last 4 bytes should be identical to the last 4 bytes of
/// FIL_PAGE_LSN.
pub const FIL_PAGE_END_LSN_OLD_CHKSUM: u32 = 8;

/// size of the page trailer.
pub const FIL_PAGE_DATA_END: u32 = 8;

/** Store the last 4 bytes of FIL_PAGE_LSN */
pub const FIL_PAGE_FCRC32_END_LSN: u32 = 8;

/** Store crc32 checksum at the end of the page */
pub const FIL_PAGE_FCRC32_CHECKSUM: u32 = 4;

/* @} */

/** File page types (values of FIL_PAGE_TYPE) @{ */
/** page_compressed, encrypted=YES (not used for full_crc32) */
pub const FIL_PAGE_PAGE_COMPRESSED_ENCRYPTED: u16 = 37401;
/** page_compressed (not used for full_crc32) */
pub const FIL_PAGE_PAGE_COMPRESSED: u16 = 34354;
/** B-tree index page */
pub const FIL_PAGE_INDEX: u16 = 17855;
/** R-tree index page (SPATIAL INDEX) */
pub const FIL_PAGE_RTREE: u16 = 17854;
/** Undo log page */
pub const FIL_PAGE_UNDO_LOG: u16 = 2;
/** Index node (of file-in-file metadata) */
pub const FIL_PAGE_INODE: u16 = 3;
/** Former change buffer free list */
pub const FIL_PAGE_IBUF_FREE_LIST: u16 = 4;
/** Freshly allocated page */
pub const FIL_PAGE_TYPE_ALLOCATED: u16 = 0;
/** Former change buffer bitmap pages (pages n*innodb_page_size+1) */
pub const FIL_PAGE_IBUF_BITMAP: u16 = 5;
/** System page */
pub const FIL_PAGE_TYPE_SYS: u16 = 6;
/** Transaction system data */
pub const FIL_PAGE_TYPE_TRX_SYS: u16 = 7;
/** Tablespace header (page 0) */
pub const FIL_PAGE_TYPE_FSP_HDR: u16 = 8;
/** Extent descriptor page (pages n*innodb_page_size, except 0) */
pub const FIL_PAGE_TYPE_XDES: u16 = 9;
/** Uncompressed BLOB page */
pub const FIL_PAGE_TYPE_BLOB: u16 = 10;
/** First ROW_FORMAT=COMPRESSED BLOB page */
pub const FIL_PAGE_TYPE_ZBLOB: u16 = 11;
/** Subsequent ROW_FORMAT=COMPRESSED BLOB page */
pub const FIL_PAGE_TYPE_ZBLOB2: u16 = 12;
/** In old tablespaces, garbage in FIL_PAGE_TYPE is replaced with this
value when flushing pages. */
pub const FIL_PAGE_TYPE_UNKNOWN: u16 = 13;

/* File page types introduced in MySQL 5.7, not supported in MariaDB */
//pub const FIL_PAGE_COMPRESSED :u16 = 14;
//pub const FIL_PAGE_ENCRYPTED :u16 = 15;
//pub const FIL_PAGE_COMPRESSED_AND_ENCRYPTED :u16 = 16;
//constexpr FIL_PAGE_ENCRYPTED_RTREE :u16 = 17;
/** Clustered index root page after instant ADD COLUMN */
pub const FIL_PAGE_TYPE_INSTANT: u16 = 18;

/** Used by i_s.cc to index into the text description.
Note: FIL_PAGE_TYPE_INSTANT maps to the same as FIL_PAGE_INDEX. */
pub const FIL_PAGE_TYPE_LAST: u16 = FIL_PAGE_TYPE_UNKNOWN;

#[allow(non_camel_case_types)]
#[derive(Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum fil_page_type_t {
    PageCompressedEncrypted = FIL_PAGE_PAGE_COMPRESSED_ENCRYPTED,
    PageCompressed = FIL_PAGE_PAGE_COMPRESSED,
    Index = FIL_PAGE_INDEX,
    RTree = FIL_PAGE_RTREE,
    UndoLog = FIL_PAGE_UNDO_LOG,
    Inode = FIL_PAGE_INODE,
    IbufFreeList = FIL_PAGE_IBUF_FREE_LIST,
    Allocated = FIL_PAGE_TYPE_ALLOCATED,
    IbufBitmap = FIL_PAGE_IBUF_BITMAP,
    Sys = FIL_PAGE_TYPE_SYS,
    TrxSys = FIL_PAGE_TYPE_TRX_SYS,
    FspHdr = FIL_PAGE_TYPE_FSP_HDR,
    Xdes = FIL_PAGE_TYPE_XDES,
    Blob = FIL_PAGE_TYPE_BLOB,
    ZBlob = FIL_PAGE_TYPE_ZBLOB,
    ZBlob2 = FIL_PAGE_TYPE_ZBLOB2,
    Unknown = FIL_PAGE_TYPE_UNKNOWN,
    Instant = FIL_PAGE_TYPE_INSTANT,
}

impl From<u16> for fil_page_type_t {
    fn from(value: u16) -> Self {
        match value {
            FIL_PAGE_PAGE_COMPRESSED_ENCRYPTED => fil_page_type_t::PageCompressedEncrypted,
            FIL_PAGE_PAGE_COMPRESSED => fil_page_type_t::PageCompressed,
            FIL_PAGE_INDEX => fil_page_type_t::Index,
            FIL_PAGE_RTREE => fil_page_type_t::RTree,
            FIL_PAGE_UNDO_LOG => fil_page_type_t::UndoLog,
            FIL_PAGE_INODE => fil_page_type_t::Inode,
            FIL_PAGE_IBUF_FREE_LIST => fil_page_type_t::IbufFreeList,
            FIL_PAGE_TYPE_ALLOCATED => fil_page_type_t::Allocated,
            FIL_PAGE_IBUF_BITMAP => fil_page_type_t::IbufBitmap,
            FIL_PAGE_TYPE_SYS => fil_page_type_t::Sys,
            FIL_PAGE_TYPE_TRX_SYS => fil_page_type_t::TrxSys,
            FIL_PAGE_TYPE_FSP_HDR => fil_page_type_t::FspHdr,
            FIL_PAGE_TYPE_XDES => fil_page_type_t::Xdes,
            FIL_PAGE_TYPE_BLOB => fil_page_type_t::Blob,
            FIL_PAGE_TYPE_ZBLOB => fil_page_type_t::ZBlob,
            FIL_PAGE_TYPE_ZBLOB2 => fil_page_type_t::ZBlob2,
            FIL_PAGE_TYPE_UNKNOWN => fil_page_type_t::Unknown,
            FIL_PAGE_TYPE_INSTANT => fil_page_type_t::Instant,
            _ => fil_page_type_t::Unknown,
        }
    }
}

/** Set in FIL_PAGE_TYPE for full_crc32 pages in page_compressed format.
If the flag is set, then the following holds for the remaining bits
of FIL_PAGE_TYPE:
Bits 0..7 will contain the compressed page size in bytes.
Bits 8..14 are reserved and must be 0. */
pub const FIL_PAGE_COMPRESS_FCRC32_MARKER: u16 = 15;
/* @} */

/// Determine if full_crc32 is used for a data file
///
/// # Arguments
/// * `flags` - tablespace flags (FSP_SPACE_FLAGS)
///
/// # Returns
/// Whether the full_crc32 algorithm is active
pub fn full_crc32(flags: u32) -> bool {
    flags & fsp0types::FSP_FLAGS_FCRC32_MASK_MARKER != 0
}

pub fn is_full_crc32_compressed(flags: u32) -> bool {
    if !full_crc32(flags) {
        return false;
    }

    let algo = fsp0types::FSP_FLAGS_FCRC32_GET_COMPRESSED_ALGO(flags);
    debug_assert!(algo <= fsp0types::PAGE_ALGORITHM_LAST);
    algo != 0
}

/// Determine the logical page size.
///
/// # Arguments
/// * `flags` - tablespace flags (FSP_SPACE_FLAGS)
///
/// # Returns
/// The logical page size, or 0 if the flags are invalid
pub fn logical_size(flags: u32) -> usize {
    let page_ssize = if full_crc32(flags) {
        fsp0types::FSP_FLAGS_FCRC32_GET_PAGE_SSIZE(flags)
    } else {
        fsp0types::FSP_FLAGS_GET_PAGE_SSIZE(flags)
    };

    match page_ssize {
        3 => 4096,
        4 => 8192,
        5 => {
            if full_crc32(flags) {
                16384
            } else {
                0
            }
        }
        0 => {
            if full_crc32(flags) {
                0
            } else {
                16384
            }
        }
        6 => 32768,
        7 => 65536,
        _ => 0,
    }
}

/// Determine the ROW_FORMAT=COMPRESSED page size.
///
/// # Arguments
/// * `flags` - tablespace flags (FSP_SPACE_FLAGS)
///
/// # Returns
/// The ROW_FORMAT=COMPRESSED page size, or 0 if not used
pub fn zip_size(flags: u32) -> u32 {
    if full_crc32(flags) {
        return 0;
    }

    let zip_ssize = fsp0types::FSP_FLAGS_GET_ZIP_SSIZE(flags);
    if zip_ssize != 0 {
        (univ::UNIV_ZIP_SIZE_MIN >> 1) << zip_ssize
    } else {
        0
    }
}

/// Determine the physical page size.
///
/// # Arguments
/// * `flags` - tablespace flags (FSP_SPACE_FLAGS)
///
/// # Returns
/// The physical page size
pub fn physical_size(flags: u32, page_size: usize) -> usize {
    if full_crc32(flags) {
        return logical_size(flags);
    }

    let zip_ssize = fsp0types::FSP_FLAGS_GET_ZIP_SSIZE(flags);
    if zip_ssize != 0 {
        ((univ::UNIV_ZIP_SIZE_MIN >> 1) << zip_ssize) as usize
    } else {
        page_size
    }
}

/// Validate the tablespace flags for full crc32 format.
///
/// # Arguments
/// * `flags` - contents of FSP_SPACE_FLAGS
///
/// # Returns
/// Whether the flags are correct in full crc32 format
pub fn is_fcrc32_valid_flags(flags: u32, page_size: usize) -> bool {
    debug_assert!(flags & fsp0types::FSP_FLAGS_FCRC32_MASK_MARKER != 0);

    let page_ssize = physical_size(flags, page_size);
    if page_ssize < 3 || (page_ssize & 8) != 0 {
        return false;
    }

    let shifted_flags = flags >> fsp0types::FSP_FLAGS_FCRC32_POS_COMPRESSED_ALGO;
    shifted_flags <= fsp0types::PAGE_ALGORITHM_LAST
}

/// Validate the tablespace flags.
///
/// # Arguments
/// * `flags` - contents of FSP_SPACE_FLAGS
/// * `is_ibd` - whether this is an .ibd file (not system tablespace)
/// * `page_size` - page size of the tablespace, typically 16K
///
/// # Returns
/// Whether the flags are correct
pub fn is_valid_flags(flags: u32, is_ibd: bool, page_size: usize) -> bool {
    if full_crc32(flags) {
        return is_fcrc32_valid_flags(flags, page_size);
    }

    if flags == 0 {
        return true;
    }
    if (flags & !fsp0types::FSP_FLAGS_MASK) != 0 {
        return false;
    }

    if (flags & (fsp0types::FSP_FLAGS_MASK_POST_ANTELOPE | fsp0types::FSP_FLAGS_MASK_ATOMIC_BLOBS))
        == fsp0types::FSP_FLAGS_MASK_ATOMIC_BLOBS
    {
        // If the "atomic blobs" flag (indicating ROW_FORMAT=DYNAMIC or ROW_FORMAT=COMPRESSED) flag is set,
        // then the ROW_FORMAT!=REDUNDANT flag must also be set.
        return false;
    }

    // Bits 10..14 should be 0b0000d where d is the DATA_DIR flag of MySQL 5.6 and MariaDB 10.0, which we ignore.
    // In the buggy FSP_SPACE_FLAGS written by MariaDB 10.1.0 to 10.1.20,
    // bits 10..14 would be nonzero 0bsssaa where sss is nonzero PAGE_SSIZE (3, 4, 6, or 7)
    // and aa is ATOMIC_WRITES (not 0b11).
    if (fsp0types::FSP_FLAGS_GET_RESERVED(flags) & !1u32) != 0 {
        return false;
    }

    let ssize = fsp0types::FSP_FLAGS_GET_PAGE_SSIZE(flags);
    if ssize == 1 || ssize == 2 || ssize == 5 || (ssize & 8) != 0 {
        // the page_size is not between 4k and 64k; 16k should be encoded as 0, not 5
        return false;
    }

    let zssize = fsp0types::FSP_FLAGS_GET_ZIP_SSIZE(flags);
    if zssize == 0 {
        // not ROW_FORMAT=COMPRESSED
    } else if zssize > if ssize != 0 { ssize } else { 5 } {
        // Invalid KEY_BLOCK_SIZE
        return false;
    } else if (!flags
        & (fsp0types::FSP_FLAGS_MASK_POST_ANTELOPE | fsp0types::FSP_FLAGS_MASK_ATOMIC_BLOBS))
        != 0
    {
        // both these flags must set for ROW_FORMAT=COMPRESSED
        return false;
    }

    // The flags do look valid. But, avoid misinterpreting
    // buggy MariaDB 10.1 format flags for PAGE_COMPRESSED=1 PAGE_COMPRESSION_LEVEL={0,2,3}
    // as valid-looking PAGE_SSIZE if this is known to be an .ibd file and we are using the default innodb_page_size=16k.
    ssize == 0 || !is_ibd || page_size != univ::UNIV_PAGE_SIZE_ORIG as usize
}

/// Returns whether the page type is B-tree or R-tree index.
#[allow(dead_code)]
fn fil_page_type_is_index(page_type: u16) -> bool {
    matches!(
        page_type,
        FIL_PAGE_TYPE_INSTANT | FIL_PAGE_INDEX | FIL_PAGE_RTREE
    )
}

/// Check whether the page is an index page (either regular Btree index or Rtree index).
#[allow(dead_code)]
fn fil_page_index_page_check(page: &[u8]) -> bool {
    fil_page_type_is_index(fil_page_get_type(page))
}

/// Get the file page type.
pub fn fil_page_get_type(page: &[u8]) -> u16 {
    mach::mach_read_from_2(&page[FIL_PAGE_TYPE as usize..])
}

pub fn tablespace_flags_to_string(flags: u32) -> String {
    let mut parts = Vec::new();

    if full_crc32(flags) {
        parts.push("FULL_CRC32".to_string());

        let pssize = fsp0types::FSP_FLAGS_FCRC32_GET_PAGE_SSIZE(flags);
        parts.push(format!("PAGE_SSIZE={}", pssize));
    } else {
        let pssize = fsp0types::FSP_FLAGS_GET_PAGE_SSIZE(flags);
        if pssize != 0 {
            parts.push(format!("PAGE_SSIZE={}", pssize));
        }

        let zssize = fsp0types::FSP_FLAGS_GET_ZIP_SSIZE(flags);
        if zssize != 0 {
            parts.push(format!("ZIP_SSIZE={}", zssize));
        }
    }

    if fsp0types::FSP_FLAGS_HAS_PAGE_COMPRESSION(flags) != 0 {
        parts.push("COMPRESSION".to_string());

        let algo = fsp0types::FSP_FLAGS_FCRC32_GET_COMPRESSED_ALGO(flags);
        if algo != 0 {
            parts.push(format!("COMPRESSION_ALGO={}", algo));
        }
    }

    if fsp0types::FSP_FLAGS_HAS_ATOMIC_BLOBS(flags) != 0 {
        parts.push("ATOMIC_BLOBS".to_string());
    }

    if fsp0types::FSP_FLAGS_GET_POST_ANTELOPE(flags) != 0 {
        parts.push("POST_ANTELOPE".to_string());
    }

    if flags & fsp0types::FSP_FLAGS_MASK_RESERVED != 0 {
        let reserved = fsp0types::FSP_FLAGS_GET_RESERVED(flags);
        parts.push(format!("RESERVED={}", reserved));
    }

    parts.push(format!("RAW=0x{:08X}", flags));

    parts.join("|")
}
