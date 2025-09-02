pub const HOSTNAME_LENGTH: u32 = 255;
pub const SYSTEM_CHARSET_MBMAXLEN: u32 = 3;
pub const NAME_CHAR_LEN: u32 = 64; /* Field/table name length */
pub const USERNAME_CHAR_LENGTH: u32 = 128;

pub const NAME_LEN: u32 = NAME_CHAR_LEN * SYSTEM_CHARSET_MBMAXLEN;

/*
            DATABASE VERSION CONTROL
            ========================
*/

/** log2 of smallest compressed page size (1<<10 == 1024 bytes)
Note: This must never change! */
pub const UNIV_ZIP_SIZE_SHIFT_MIN: u32 = 10;

/** log2 of largest compressed page size (1<<14 == 16384 bytes).
A compressed page directory entry reserves 14 bits for the start offset
and 2 bits for flags. This limits the uncompressed page size to 16k.
*/
pub const UNIV_ZIP_SIZE_SHIFT_MAX: u32 = 14;

/* Define the Min, Max, Default page sizes. */
/** Minimum Page Size Shift (power of 2) */
pub const UNIV_PAGE_SIZE_SHIFT_MIN: u32 = 12;
/** log2 of largest page size (1<<16 == 65536 bytes). */
/** Maximum Page Size Shift (power of 2) */
pub const UNIV_PAGE_SIZE_SHIFT_MAX: u32 = 16;
/** log2 of default page size (1<<14 == 16384 bytes). */
/** Default Page Size Shift (power of 2) */
pub const UNIV_PAGE_SIZE_SHIFT_DEF: u32 = 14;
/** Original 16k InnoDB Page Size Shift, in case the default changes */
pub const UNIV_PAGE_SIZE_SHIFT_ORIG: u32 = 14;
/** Original 16k InnoDB Page Size as an ssize (log2 - 9) */
pub const UNIV_PAGE_SSIZE_ORIG: u32 = UNIV_PAGE_SIZE_SHIFT_ORIG - 9;

/** Minimum page size InnoDB currently supports. */
pub const UNIV_PAGE_SIZE_MIN: u32 = 1u32 << UNIV_PAGE_SIZE_SHIFT_MIN;
/** Maximum page size InnoDB currently supports. */
pub const UNIV_PAGE_SIZE_MAX: u32 = 1u32 << UNIV_PAGE_SIZE_SHIFT_MAX;
/** Default page size for InnoDB tablespaces. */
pub const UNIV_PAGE_SIZE_DEF: u32 = 1u32 << UNIV_PAGE_SIZE_SHIFT_DEF;
/** Original 16k page size for InnoDB tablespaces. */
pub const UNIV_PAGE_SIZE_ORIG: u32 = 1u32 << UNIV_PAGE_SIZE_SHIFT_ORIG;

/** Smallest compressed page size */
pub const UNIV_ZIP_SIZE_MIN: u32 = 1u32 << UNIV_ZIP_SIZE_SHIFT_MIN;

/** Largest compressed page size */
pub const UNIV_ZIP_SIZE_MAX: u32 = 1u32 << UNIV_ZIP_SIZE_SHIFT_MAX;

/** Largest possible ssize for an uncompressed page.
The convention 'ssize' is used for 'log2 minus 9' or the number of
shifts starting with 512.
This max number varies depending on srv_page_size. */
pub fn univ_page_ssize_max(page_size_shift: u32) -> u32 {
    page_size_shift - UNIV_ZIP_SIZE_SHIFT_MIN + 1
}

/** Smallest possible ssize for an uncompressed page. */
pub const UNIV_PAGE_SSIZE_MIN: u32 = UNIV_PAGE_SIZE_SHIFT_MIN - UNIV_ZIP_SIZE_SHIFT_MIN + 1;

/** Maximum number of parallel threads in a parallelized operation */
pub const UNIV_MAX_PARALLELISM: u32 = 32;

/** This is the "mbmaxlen" for my_charset_filename (defined in
strings/ctype-utf8.c), which is used to encode File and Database names. */
pub const FILENAME_CHARSET_MAXNAMLEN: u32 = 5;

/** The maximum length of an encode table name in bytes.  The max
table and database names are NAME_CHAR_LEN (64) characters. After the
encoding, the max length would be NAME_CHAR_LEN (64) *
FILENAME_CHARSET_MAXNAMLEN (5) = 320 bytes. The number does not include a
terminating '\0'. InnoDB can handle longer names internally */
pub const MAX_TABLE_NAME_LEN: u32 = 320;

/** The maximum length of a database name. Like MAX_TABLE_NAME_LEN this is
the MySQL's NAME_LEN, see check_and_convert_db_name(). */
pub const MAX_DATABASE_NAME_LEN: u32 = MAX_TABLE_NAME_LEN;

/** MAX_FULL_NAME_LEN defines the full name path including the
database name and table name. In addition, 14 bytes is added for:
    2 for surrounding quotes around table name
    1 for the separating dot (.)
    9 for the #mysql50# prefix */
pub const MAX_FULL_NAME_LEN: u32 = MAX_TABLE_NAME_LEN + MAX_DATABASE_NAME_LEN + 14;

/** Maximum length of the compression alogrithm string. Currently we support
only (NONE | ZLIB | LZ4). */
pub const MAX_COMPRESSION_LEN: u32 = 4;

/** The maximum length in bytes that a database name can occupy when stored in
UTF8, including the terminating '\0', see dict_fs2utf8(). You must include
mysql_com.h if you are to use this macro. */
pub const MAX_DB_UTF8_LEN: u32 = NAME_LEN + 1;

// The maximum length in bytes that a table name can occupy when stored in
// UTF8, including the terminating '\0', see dict_fs2utf8(). You must include
// mysql_com.h if you are to use this macro.
// pub const MAX_TABLE_UTF8_LEN	:u32=(NAME_LEN + sizeof(srv_mysql50_table_name_prefix));

/// log2 of the page size (14 for 1<<14 == 16384 bytes).
pub fn page_size_shift(page_size: u32) -> u32 {
    match page_size {
        // 16 is the max ([`UNIV_PAGE_SIZE_SHIFT_MAX`])
        65536 => 16,
        32768 => 15,
        16384 => 14,
        8192 => 13,
        4096 => 12,
        // 12 is the min ([`UNIV_PAGE_SIZE_SHIFT_MIN`])
        _ => panic!("Invalid page size: {}", page_size),
    }
}

/*
            UNIVERSAL TYPE DEFINITIONS
            ==========================
*/

/** The bitmask of 32-bit unsigned integer */
pub const ULINT32_MASK: u32 = 0xFFFFFFFFu32;
/** The undefined 32-bit unsigned integer */
pub const ULINT32_UNDEFINED: u32 = ULINT32_MASK;
