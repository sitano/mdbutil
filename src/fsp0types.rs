use crate::fil0fil;
use crate::univ;

/** All persistent tablespaces have a smaller fil_space_t::id than this. */
pub const SRV_SPACE_ID_UPPER_BOUND: u32 = 0xFFFFFFF0u32;

/** The fil_space_t::id of the innodb_temporary tablespace. */
pub const SRV_TMP_SPACE_ID: u32 = 0xFFFFFFFEu32;

/* Possible values of innodb_compression_algorithm */
pub const PAGE_UNCOMPRESSED: u32 = 0;
pub const PAGE_ZLIB_ALGORITHM: u32 = 1;
pub const PAGE_LZ4_ALGORITHM: u32 = 2;
pub const PAGE_LZO_ALGORITHM: u32 = 3;
pub const PAGE_LZMA_ALGORITHM: u32 = 4;
pub const PAGE_BZIP2_ALGORITHM: u32 = 5;
pub const PAGE_SNAPPY_ALGORITHM: u32 = 6;
pub const PAGE_ALGORITHM_LAST: u32 = PAGE_SNAPPY_ALGORITHM;

/** @name Flags for inserting records in order
If records are inserted in order, there are the following
flags to tell this (their type is made byte for the compiler
to warn if direction and hint parameters are switched in
fseg_alloc_free_page_general) */
pub const FSP_UP: u8 = 111; // alphabetically upwards
pub const FSP_DOWN: u8 = 112; // alphabetically downwards
pub const FSP_NO_DIR: u8 = 113; // no order

/** File space extent size in pages
page size | file space extent size
----------+-----------------------
   4 KiB  | 256 pages = 1 MiB
   8 KiB  | 128 pages = 1 MiB
  16 KiB  |  64 pages = 1 MiB
  32 KiB  |  64 pages = 2 MiB
  64 KiB  |  64 pages = 4 MiB

  page_size_shift = log2(page_size).
*/
#[allow(non_snake_case)]
pub fn FSP_EXTENT_SIZE(page_size_shift: u32) -> u32 {
    if page_size_shift < 14 {
        1048576u32 >> page_size_shift
    } else {
        64u32
    }
}

/** File space extent size (four megabyte) in pages for MAX page size */
pub const FSP_EXTENT_SIZE_MAX: u32 = 4194304 / univ::UNIV_PAGE_SIZE_MAX;

/** File space extent size (one megabyte) in pages for MIN page size */
pub const FSP_EXTENT_SIZE_MIN: u32 = 1048576 / univ::UNIV_PAGE_SIZE_MIN;

/** On a page of any file segment, data may be put starting from this
offset */
pub const FSEG_PAGE_DATA: u32 = fil0fil::FIL_PAGE_DATA;

/** @name File segment header
The file segment header points to the inode describing the file segment. */
/* @{ */

/** Data type for file segment header */
#[allow(non_camel_case_types)]
pub type fseg_header_t = u8;

/// space id of the inode.
pub const FSEG_HDR_SPACE: u8 = 0;

/// page number of the inode.
pub const FSEG_HDR_PAGE_NO: u8 = 4;

/// byte offset of the inode.
pub const FSEG_HDR_OFFSET: u8 = 8;

/// Length of the file system header, in bytes.
pub const FSEG_HEADER_SIZE: u8 = 10;

/* @} */

/** Flags for fsp_reserve_free_extents */
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum fsp_reserve_t {
    FSP_NORMAL,   /* reservation during normal B-tree operations */
    FSP_UNDO,     /* reservation done for undo logging */
    FSP_CLEANING, /* reservation done during purge operations */
    FSP_BLOB,     /* reservation being done for BLOB insertion */
}

/* Number of pages described in a single descriptor page: currently each page
description takes less than 1 byte; a descriptor page is repeated every
this many file pages */
/* #define XDES_DESCRIBED_PER_PAGE		srv_page_size */
/* This has been replaced with either srv_page_size or page_zip->size. */

/** @name The space low address page map
The 2 pages at FSP_XDES_OFFSET are repeated
every XDES_DESCRIBED_PER_PAGE pages in every tablespace. */
/* @{ */

/// extent descriptor in every tablespace.
pub const FSP_XDES_OFFSET: u32 = 0;
/// the following pages exist in the system tablespace (space 0).
pub const FSP_FIRST_INODE_PAGE_NO: u32 = 2;
///former change buffer header page, in tablespace 0.
pub const FSP_IBUF_HEADER_PAGE_NO: u32 = 3;
/// former change buffer B-tree root page in tablespace 0.
pub const FSP_IBUF_TREE_ROOT_PAGE_NO: u32 = 4;
/// transaction system header, in tablespace 0.
pub const FSP_TRX_SYS_PAGE_NO: u32 = 5;
/// first rollback segment page, in tablespace 0.
pub const FSP_FIRST_RSEG_PAGE_NO: u32 = 6;
/// data dictionary header page, in tablespace 0.
pub const FSP_DICT_HDR_PAGE_NO: u32 = 7;

/* @} */

/// Check if tablespace is system temporary.
/// space_id - verify is checksum is enabled for given space.
/// returns true if tablespace is system temporary.
#[allow(non_snake_case)]
pub fn FSP_IS_SYSTEM_TEMPORARY(space_id: u32) -> bool {
    return space_id == SRV_TMP_SPACE_ID;
}

/* @defgroup fsp_flags InnoDB Tablespace Flag Constants @{ */

/** Width of the POST_ANTELOPE flag */
pub const FSP_FLAGS_WIDTH_POST_ANTELOPE: u32 = 1;
/** Number of flag bits used to indicate the tablespace zip page size */
pub const FSP_FLAGS_WIDTH_ZIP_SSIZE: u32 = 4;
/** Width of the ATOMIC_BLOBS flag.  The ability to break up a long
column into an in-record prefix and an externally stored part is available
to ROW_FORMAT=REDUNDANT and ROW_FORMAT=COMPACT. */
pub const FSP_FLAGS_WIDTH_ATOMIC_BLOBS: u32 = 1;
/** Number of flag bits used to indicate the tablespace page size */
pub const FSP_FLAGS_WIDTH_PAGE_SSIZE: u32 = 4;
/** Number of reserved bits */
pub const FSP_FLAGS_WIDTH_RESERVED: u32 = 6;
/** Number of flag bits used to indicate the page compression */
pub const FSP_FLAGS_WIDTH_PAGE_COMPRESSION: u32 = 1;

/** Width of all the currently known persistent tablespace flags */
pub const FSP_FLAGS_WIDTH: u32 = FSP_FLAGS_WIDTH_POST_ANTELOPE
    + FSP_FLAGS_WIDTH_ZIP_SSIZE
    + FSP_FLAGS_WIDTH_ATOMIC_BLOBS
    + FSP_FLAGS_WIDTH_PAGE_SSIZE
    + FSP_FLAGS_WIDTH_RESERVED
    + FSP_FLAGS_WIDTH_PAGE_COMPRESSION;

/** A mask of all the known/used bits in FSP_SPACE_FLAGS */
pub const FSP_FLAGS_MASK: u32 = !(!0u32 << FSP_FLAGS_WIDTH);

/** Number of flag bits used to indicate the tablespace page size */
pub const FSP_FLAGS_FCRC32_WIDTH_PAGE_SSIZE: u32 = 4;

/** Marker to indicate whether tablespace is in full checksum format. */
pub const FSP_FLAGS_FCRC32_WIDTH_MARKER: u32 = 1;

/** Stores the compressed algo for full checksum format. */
pub const FSP_FLAGS_FCRC32_WIDTH_COMPRESSED_ALGO: u32 = 3;

/* FSP_SPACE_FLAGS position and name in MySQL 5.6/MariaDB 10.0 or older
and MariaDB 10.1.20 or older MariaDB 10.1 and in MariaDB 10.1.21
or newer.
MySQL 5.6		MariaDB 10.1.x		MariaDB 10.1.21
====================================================================
Below flags in same offset
====================================================================
0: POST_ANTELOPE	0:POST_ANTELOPE		0: POST_ANTELOPE
1..4: ZIP_SSIZE(0..5)	1..4:ZIP_SSIZE(0..5)	1..4: ZIP_SSIZE(0..5)
(NOTE: bit 4 is always 0)
5: ATOMIC_BLOBS    	5:ATOMIC_BLOBS		5: ATOMIC_BLOBS
=====================================================================
Below note the order difference:
=====================================================================
6..9: PAGE_SSIZE(3..7)	6: COMPRESSION		6..9: PAGE_SSIZE(3..7)
10: DATA_DIR		7..10: COMP_LEVEL(0..9)	10: RESERVED (5.6 DATA_DIR)
=====================================================================
The flags below were in incorrect position in MariaDB 10.1,
or have been introduced in MySQL 5.7 or 8.0:
=====================================================================
11: UNUSED		11..12:ATOMIC_WRITES	11: RESERVED (5.7 SHARED)
                        12: RESERVED (5.7 TEMPORARY)
            13..15:PAGE_SSIZE(3..7)	13: RESERVED (5.7 ENCRYPTION)
                        14: RESERVED (8.0 SDI)
                        15: RESERVED
            16: PAGE_SSIZE_msb(0)	16: COMPRESSION
            17: DATA_DIR		17: UNUSED
            18: UNUSED
=====================================================================
The flags below only exist in fil_space_t::flags, not in FSP_SPACE_FLAGS:
=====================================================================
                        27: DATA_DIR
                        28..31: COMPRESSION_LEVEL
*/

/** A mask of the memory-only flags in fil_space_t::flags */
pub const FSP_FLAGS_MEM_MASK: u32 = !0u32 << FSP_FLAGS_MEM_DATA_DIR;

/** Zero relative shift position of the DATA_DIR flag */
pub const FSP_FLAGS_MEM_DATA_DIR: u32 = 27;
/** Zero relative shift position of the COMPRESSION_LEVEL field */
pub const FSP_FLAGS_MEM_COMPRESSION_LEVEL: u32 = 28;

/** Zero relative shift position of the POST_ANTELOPE field */
pub const FSP_FLAGS_POS_POST_ANTELOPE: u32 = 0;
/** Zero relative shift position of the ZIP_SSIZE field */
pub const FSP_FLAGS_POS_ZIP_SSIZE: u32 =
    FSP_FLAGS_POS_POST_ANTELOPE + FSP_FLAGS_WIDTH_POST_ANTELOPE;
/** Zero relative shift position of the ATOMIC_BLOBS field */
pub const FSP_FLAGS_POS_ATOMIC_BLOBS: u32 = FSP_FLAGS_POS_ZIP_SSIZE + FSP_FLAGS_WIDTH_ZIP_SSIZE;
/** Zero relative shift position of the start of the PAGE_SSIZE bits */
pub const FSP_FLAGS_POS_PAGE_SSIZE: u32 = FSP_FLAGS_POS_ATOMIC_BLOBS + FSP_FLAGS_WIDTH_ATOMIC_BLOBS;
/** Zero relative shift position of the start of the RESERVED bits
these are only used in MySQL 5.7 and used for compatibility. */
pub const FSP_FLAGS_POS_RESERVED: u32 = FSP_FLAGS_POS_PAGE_SSIZE + FSP_FLAGS_WIDTH_PAGE_SSIZE;
/** Zero relative shift position of the PAGE_COMPRESSION field */
pub const FSP_FLAGS_POS_PAGE_COMPRESSION: u32 = FSP_FLAGS_POS_RESERVED + FSP_FLAGS_WIDTH_RESERVED;

/** Zero relative shift position of the PAGE_SIZE field
in full crc32 format */
pub const FSP_FLAGS_FCRC32_POS_PAGE_SSIZE: u32 = 0;

/** Zero relative shift position of the MARKER field in full crc32 format. */
pub const FSP_FLAGS_FCRC32_POS_MARKER: u32 =
    FSP_FLAGS_FCRC32_POS_PAGE_SSIZE + FSP_FLAGS_FCRC32_WIDTH_PAGE_SSIZE;

/** Zero relative shift position of the compressed algorithm stored
in full crc32 format. */
pub const FSP_FLAGS_FCRC32_POS_COMPRESSED_ALGO: u32 =
    FSP_FLAGS_FCRC32_POS_MARKER + FSP_FLAGS_FCRC32_WIDTH_MARKER;

/** Bit mask of the POST_ANTELOPE field */
pub const FSP_FLAGS_MASK_POST_ANTELOPE: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_POST_ANTELOPE)) << FSP_FLAGS_POS_POST_ANTELOPE;
/** Bit mask of the ZIP_SSIZE field */
pub const FSP_FLAGS_MASK_ZIP_SSIZE: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_ZIP_SSIZE)) << FSP_FLAGS_POS_ZIP_SSIZE;
/** Bit mask of the ATOMIC_BLOBS field */
pub const FSP_FLAGS_MASK_ATOMIC_BLOBS: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_ATOMIC_BLOBS)) << FSP_FLAGS_POS_ATOMIC_BLOBS;
/** Bit mask of the PAGE_SSIZE field */
pub const FSP_FLAGS_MASK_PAGE_SSIZE: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_PAGE_SSIZE)) << FSP_FLAGS_POS_PAGE_SSIZE;
/** Bit mask of the RESERVED1 field */
pub const FSP_FLAGS_MASK_RESERVED: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_RESERVED)) << FSP_FLAGS_POS_RESERVED;
/** Bit mask of the PAGE_COMPRESSION field */
pub const FSP_FLAGS_MASK_PAGE_COMPRESSION: u32 =
    (!(!0u32 << FSP_FLAGS_WIDTH_PAGE_COMPRESSION)) << FSP_FLAGS_POS_PAGE_COMPRESSION;

/** Bit mask of the in-memory COMPRESSION_LEVEL field */
pub const FSP_FLAGS_MASK_MEM_COMPRESSION_LEVEL: u32 = 15u32 << FSP_FLAGS_MEM_COMPRESSION_LEVEL;

/** Bit mask of the PAGE_SIZE field in full crc32 format */
pub const FSP_FLAGS_FCRC32_MASK_PAGE_SSIZE: u32 =
    (!(!0u32 << FSP_FLAGS_FCRC32_WIDTH_PAGE_SSIZE)) << FSP_FLAGS_FCRC32_POS_PAGE_SSIZE;

/** Bit mask of the MARKER field in full crc32 format */
pub const FSP_FLAGS_FCRC32_MASK_MARKER: u32 =
    (!(!0u32 << FSP_FLAGS_FCRC32_WIDTH_MARKER)) << FSP_FLAGS_FCRC32_POS_MARKER;

/** Bit mask of the COMPRESSED ALGO field in full crc32 format */
pub const FSP_FLAGS_FCRC32_MASK_COMPRESSED_ALGO: u32 =
    (!(!0u32 << FSP_FLAGS_FCRC32_WIDTH_COMPRESSED_ALGO)) << FSP_FLAGS_FCRC32_POS_COMPRESSED_ALGO;

/** Return the value of the POST_ANTELOPE field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_POST_ANTELOPE(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_POST_ANTELOPE) >> FSP_FLAGS_POS_POST_ANTELOPE
}
/** Return the value of the ZIP_SSIZE field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_ZIP_SSIZE(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_ZIP_SSIZE) >> FSP_FLAGS_POS_ZIP_SSIZE
}
/** Return the value of the ATOMIC_BLOBS field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_HAS_ATOMIC_BLOBS(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_ATOMIC_BLOBS) >> FSP_FLAGS_POS_ATOMIC_BLOBS
}
/** Return the value of the PAGE_SSIZE field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_PAGE_SSIZE(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_PAGE_SSIZE) >> FSP_FLAGS_POS_PAGE_SSIZE
}
/** @return the RESERVED flags */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_RESERVED(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_RESERVED) >> FSP_FLAGS_POS_RESERVED
}
/** @return the PAGE_COMPRESSION flag */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_HAS_PAGE_COMPRESSION(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_PAGE_COMPRESSION) >> FSP_FLAGS_POS_PAGE_COMPRESSION
}
/** @return the PAGE_SSIZE flags in full crc32 format */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_FCRC32_GET_PAGE_SSIZE(flags: u32) -> u32 {
    (flags & FSP_FLAGS_FCRC32_MASK_PAGE_SSIZE) >> FSP_FLAGS_FCRC32_POS_PAGE_SSIZE
}
/** @return the COMPRESSED_ALGO flags in full crc32 format */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_FCRC32_GET_COMPRESSED_ALGO(flags: u32) -> u32 {
    (flags & FSP_FLAGS_FCRC32_MASK_COMPRESSED_ALGO) >> FSP_FLAGS_FCRC32_POS_COMPRESSED_ALGO
}

/** @return the value of the DATA_DIR field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_HAS_DATA_DIR(flags: u32) -> u32 {
    flags & 1u32 << FSP_FLAGS_MEM_DATA_DIR
}
/** @return the COMPRESSION_LEVEL field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_PAGE_COMPRESSION_LEVEL(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_MEM_COMPRESSION_LEVEL) >> FSP_FLAGS_MEM_COMPRESSION_LEVEL
}

/* @} */
