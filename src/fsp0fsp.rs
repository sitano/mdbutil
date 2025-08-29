use crate::fil0fil;
use crate::fsp0types;
use crate::fut0lst;
use crate::univ;
use crate::ut0ut::UT_BITS_IN_BYTES;

/// @return the PAGE_SSIZE flags for the current innodb_page_size.
#[allow(non_snake_case)]
pub fn FSP_FLAGS_PAGE_SSIZE(page_size: usize, page_size_shift: usize) -> u32 {
    if page_size == univ::UNIV_PAGE_SIZE_ORIG as usize {
        0u32
    } else {
        ((page_size_shift - univ::UNIV_ZIP_SIZE_SHIFT_MIN as usize + 1)
            << fsp0types::FSP_FLAGS_POS_PAGE_SSIZE) as u32
    }
}

/// @return the PAGE_SSIZE flags for the current innodb_page_size in full checksum format.
#[allow(non_snake_case)]
pub fn FSP_FLAGS_FCRC32_PAGE_SSIZE(page_size_shift: usize) -> u32 {
    ((page_size_shift - univ::UNIV_ZIP_SIZE_SHIFT_MIN as usize + 1)
        << fsp0types::FSP_FLAGS_FCRC32_POS_PAGE_SSIZE) as u32
}

/// @defgroup Compatibility macros for MariaDB 10.1.0 through 10.1.20; see the table in fsp0types.h
/// @{ */
/// Zero relative shift position of the PAGE_COMPRESSION field.
pub const FSP_FLAGS_POS_PAGE_COMPRESSION_MARIADB101: u32 =
    fsp0types::FSP_FLAGS_POS_ATOMIC_BLOBS + fsp0types::FSP_FLAGS_WIDTH_ATOMIC_BLOBS;
/// Zero relative shift position of the PAGE_COMPRESSION_LEVEL field.
pub const FSP_FLAGS_POS_PAGE_COMPRESSION_LEVEL_MARIADB101: u32 =
    FSP_FLAGS_POS_PAGE_COMPRESSION_MARIADB101 + 1;
/// Zero relative shift position of the ATOMIC_WRITES field.
pub const FSP_FLAGS_POS_ATOMIC_WRITES_MARIADB101: u32 =
    FSP_FLAGS_POS_PAGE_COMPRESSION_LEVEL_MARIADB101 + 4;
/// Zero relative shift position of the PAGE_SSIZE field.
pub const FSP_FLAGS_POS_PAGE_SSIZE_MARIADB101: u32 = FSP_FLAGS_POS_ATOMIC_WRITES_MARIADB101 + 2;

/// Bit mask of the PAGE_COMPRESSION field */
pub const FSP_FLAGS_MASK_PAGE_COMPRESSION_MARIADB101: u32 =
    1u32 << FSP_FLAGS_POS_PAGE_COMPRESSION_MARIADB101;
/// Bit mask of the PAGE_COMPRESSION_LEVEL field */
pub const FSP_FLAGS_MASK_PAGE_COMPRESSION_LEVEL_MARIADB101: u32 =
    15u32 << FSP_FLAGS_POS_PAGE_COMPRESSION_LEVEL_MARIADB101;
/// Bit mask of the ATOMIC_WRITES field */
pub const FSP_FLAGS_MASK_ATOMIC_WRITES_MARIADB101: u32 =
    3u32 << FSP_FLAGS_POS_ATOMIC_WRITES_MARIADB101;
/// Bit mask of the PAGE_SSIZE field */
pub const FSP_FLAGS_MASK_PAGE_SSIZE_MARIADB101: u32 = 15u32 << FSP_FLAGS_POS_PAGE_SSIZE_MARIADB101;

/// Return the value of the PAGE_COMPRESSION field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_PAGE_COMPRESSION_MARIADB101(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_PAGE_COMPRESSION_MARIADB101)
        >> FSP_FLAGS_POS_PAGE_COMPRESSION_MARIADB101
}
/// Return the value of the PAGE_COMPRESSION_LEVEL field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_PAGE_COMPRESSION_LEVEL_MARIADB101(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_PAGE_COMPRESSION_LEVEL_MARIADB101)
        >> FSP_FLAGS_POS_PAGE_COMPRESSION_LEVEL_MARIADB101
}
/// Return the value of the PAGE_SSIZE field */
#[allow(non_snake_case)]
pub fn FSP_FLAGS_GET_PAGE_SSIZE_MARIADB101(flags: u32) -> u32 {
    (flags & FSP_FLAGS_MASK_PAGE_SSIZE_MARIADB101) >> FSP_FLAGS_POS_PAGE_SSIZE_MARIADB101
}

/* @} */

/* @defgroup Tablespace Header Constants (moved from fsp0fsp.c) @{ */

/// Offset of the space header within a file page */
pub const FSP_HEADER_OFFSET: u32 = fil0fil::FIL_PAGE_DATA;

/* The data structures in files are defined just as byte strings in C */
#[allow(non_camel_case_types)]
#[allow(dead_code)]
type xdes_t = u8;

/*			SPACE HEADER
            ============

File space header data structure: this data structure is contained in the
first page of a space. The space for this header is reserved in every extent
descriptor page, but used only in the first. */

/*-------------------------------------*/
/// space id
pub const FSP_SPACE_ID: u32 = 0;
/// this field contained a value up to which we know that the
/// modifications in the database have been flushed to the file space; not used now
pub const FSP_NOT_USED: u32 = 4;
/// Current size of the space in pages
pub const FSP_SIZE: u32 = 8;
/// Minimum page number for which the free list has not been
/// initialized: the pages >= this limit are, by definition, free; note that in a single-table
/// tablespace where size < 64 pages, this number is 64, i.e., we have initialized the space about
/// the first extent, but have not physically allocated those pages to the file
pub const FSP_FREE_LIMIT: u32 = 12;
/// fsp_space_t.flags, similar to dict_table_t::flags
pub const FSP_SPACE_FLAGS: u32 = 16;
/// number of used pages in the FSP_FREE_FRAG list
pub const FSP_FRAG_N_USED: u32 = 20;
/// list of free extents
pub const FSP_FREE: u32 = 24;
/// list of partially free extents not belonging to any segment
pub const FSP_FREE_FRAG: u32 = 24 + fut0lst::FLST_BASE_NODE_SIZE;
/// list of full extents not belonging to any segment
pub const FSP_FULL_FRAG: u32 = 24 + 2 * fut0lst::FLST_BASE_NODE_SIZE;
/// 8 bytes which give the first unused segment id
pub const FSP_SEG_ID: u32 = 24 + 3 * fut0lst::FLST_BASE_NODE_SIZE;
/// list of pages containing segment headers, where all the segment inode slots are
/// reserved
pub const FSP_SEG_INODES_FULL: u32 = 32 + 3 * fut0lst::FLST_BASE_NODE_SIZE;
/// list of pages containing segment headers, where not all the segment header slots are
/// reserved
pub const FSP_SEG_INODES_FREE: u32 = 32 + 4 * fut0lst::FLST_BASE_NODE_SIZE;

/// File space header size
pub const FSP_HEADER_SIZE: u32 = 32 + 5 * fut0lst::FLST_BASE_NODE_SIZE;

/// this many free extents are added to the free list from above
/// FSP_FREE_LIMIT at a time
pub const FSP_FREE_ADD: u32 = 4;
/* @} */

/* @defgroup File Segment Inode Constants (moved from fsp0fsp.c) @{ */

/*			FILE SEGMENT INODE
            ==================

Segment inode which is created for each segment in a tablespace. NOTE: in
purge we assume that a segment having only one currently used page can be
freed in a few steps, so that the freeing cannot fill the file buffer with
bufferfixed file pages. */

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type fseg_inode_t = u8;

/// the list node for linking segment inode pages
pub const FSEG_INODE_PAGE_NODE: u32 = fsp0types::FSEG_PAGE_DATA;

pub const FSEG_ARR_OFFSET: u32 = fsp0types::FSEG_PAGE_DATA + fut0lst::FLST_NODE_SIZE;

// -------------------------------------

/// 8 bytes of segment id: if this is 0, it means that the header is unused
pub const FSEG_ID: u32 = 0;
/// number of used segment pages in the FSEG_NOT_FULL list
pub const FSEG_NOT_FULL_N_USED: u32 = 8;
/// list of free extents of this segment
pub const FSEG_FREE: u32 = 12;
/// list of partially free extents
pub const FSEG_NOT_FULL: u32 = 12 + fut0lst::FLST_BASE_NODE_SIZE;
/// list of full extents
pub const FSEG_FULL: u32 = 12 + 2 * fut0lst::FLST_BASE_NODE_SIZE;
/// magic number used in debugging
pub const FSEG_MAGIC_N: u32 = 12 + 3 * fut0lst::FLST_BASE_NODE_SIZE;
/// array of individual pages belonging to this segment in fsp fragment extent lists
pub const FSEG_FRAG_ARR: u32 = 16 + 3 * fut0lst::FLST_BASE_NODE_SIZE;
/// number of slots in the array for the fragment pages
#[allow(non_snake_case)]
pub fn FSEG_FRAG_ARR_N_SLOTS(page_size_shift: u32) -> u32 {
    fsp0types::FSP_EXTENT_SIZE(page_size_shift) / 2
}
/// a fragment page slot contains its page number within space,
/// FIL_NULL means that the slot is not in use
pub const FSEG_FRAG_SLOT_SIZE: u32 = 4;
///-------------------------------------*/
#[allow(non_snake_case)]
pub fn FSEG_INODE_SIZE(page_size_shift: u32) -> u32 {
    16 + 3 * fut0lst::FLST_BASE_NODE_SIZE
        + FSEG_FRAG_ARR_N_SLOTS(page_size_shift) * FSEG_FRAG_SLOT_SIZE
}

pub static FSEG_MAGIC_N_BYTES: [u8; 4] = [0x05, 0xd6, 0x69, 0xd2];

/// If the reserved size of a segment is at least this many
/// extents, we allow extents to be put to the free list of the extent: at most
/// FSEG_FREE_LIST_MAX_LEN many
pub const FSEG_FREE_LIST_LIMIT: u32 = 40;
pub const FSEG_FREE_LIST_MAX_LEN: u32 = 4;
// @}

/* @defgroup Extent Descriptor Constants (moved from fsp0fsp.c) @{ */

/*			EXTENT DESCRIPTOR
            =================

File extent descriptor data structure: contains bits to tell which pages in
the extent are free and which contain old tuple version to clean. */

/*-------------------------------------*/
pub const XDES_ID: u32 = 0; /* The identifier of the segment
to which this extent belongs */
pub const XDES_FLST_NODE: u32 = 8; /* The list node data structure
for the descriptors */
pub const XDES_STATE: u32 = fut0lst::FLST_NODE_SIZE + 8;
/* contains state information
of the extent */
pub const XDES_BITMAP: u32 = fut0lst::FLST_NODE_SIZE + 12;
/* Descriptor bitmap of the pages
in the extent */
/*-------------------------------------*/

pub const XDES_BITS_PER_PAGE: u32 = 2; /* How many bits are there per page */
pub const XDES_FREE_BIT: u32 = 0; /* Index of the bit which tells if
the page is free */
pub const XDES_CLEAN_BIT: u32 = 1; /* NOTE: currently not used!
Index of the bit which tells if
there are old versions of tuples
on the page */
/* States of a descriptor */
pub const XDES_FREE: u32 = 1; /* extent is in free list of space */
pub const XDES_FREE_FRAG: u32 = 2; /* extent is in free fragment list of
space */
pub const XDES_FULL_FRAG: u32 = 3; /* extent is in full fragment list of
space */
pub const XDES_FSEG: u32 = 4; /* extent belongs to a segment */

/// File extent data structure size in bytes. */
#[allow(non_snake_case)]
pub fn XDES_SIZE(page_size_shift: u32) -> u32 {
    XDES_BITMAP + UT_BITS_IN_BYTES(fsp0types::FSP_EXTENT_SIZE(page_size_shift) * XDES_BITS_PER_PAGE)
}

/// File extent data structure size in bytes for MAX page size. */
pub const XDES_SIZE_MAX: u32 =
    XDES_BITMAP + UT_BITS_IN_BYTES(fsp0types::FSP_EXTENT_SIZE_MAX * XDES_BITS_PER_PAGE);

/// File extent data structure size in bytes for MIN page size. */
pub const XDES_SIZE_MIN: u32 =
    XDES_BITMAP + UT_BITS_IN_BYTES(fsp0types::FSP_EXTENT_SIZE_MIN * XDES_BITS_PER_PAGE);

/// Offset of the descriptor array on a descriptor page */
pub const XDES_ARR_OFFSET: u32 = FSP_HEADER_OFFSET + FSP_HEADER_SIZE;
