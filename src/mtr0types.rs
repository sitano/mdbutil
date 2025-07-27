/// Redo log record types. These bit patterns (3 bits) will be written
/// to the redo log file, so the existing codes or their interpretation on
/// crash recovery must not be changed.
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum mrec_type_t {
    /** Free a page. On recovery, it is unnecessary to read the page.
    The next record for the page (if any) must be INIT_PAGE.
    After this record has been written, the page may be
    overwritten with zeros, or discarded or trimmed. */
    FREE_PAGE = 0,
    /** Zero-initialize a page. The current byte offset (for subsequent
    records) will be reset to FIL_PAGE_TYPE. */
    INIT_PAGE = 0x10,
    /** Extended record; @see mrec_ext_t */
    EXTENDED = 0x20,
    /** Write a string of bytes. Followed by the byte offset (unsigned,
    relative to the current byte offset, encoded in 1 to 3 bytes) and
    the bytes to write (at least one). The current byte offset will be
    set after the last byte written. */
    WRITE = 0x30,
    /** Like WRITE, but before the bytes to write, the data_length-1
    (encoded in 1 to 3 bytes) will be encoded, and it must be more
    than the length of the following data bytes to write.
    The data byte(s) will be repeatedly copied to the output until
    the data_length is reached. */
    MEMSET = 0x40,
    /** Like MEMSET, but instead of the bytes to write, a source byte
    offset (signed, nonzero, relative to the target byte offset, encoded
    in 1 to 3 bytes, with the sign bit in the least significant bit)
    will be written.
    That is, +x is encoded as (x-1)<<1 (+1,+2,+3,... is 0,2,4,...)
    and -x is encoded as (x-1)<<1|1 (-1,-2,-3,... is 1,3,5,...).
    The source offset and data_length must be within the page size, or
    else the record will be treated as corrupted. The data will be
    copied from the page as it was at the start of the
    mini-transaction. */
    MEMMOVE = 0x50,
    /** Reserved for future use. */
    RESERVED = 0x60,
    /** Optional record that may be ignored in crash recovery.
    A subtype (@see mrec_opt) will be encoded after the page identifier. */
    OPTION = 0x70,
}

/// Redo log record types for file-level operations. These bit
/// patterns will be written to redo log files, so the existing codes or
/// their interpretation on crash recovery must not be changed.
#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum mfile_type_t {
    /** Create a file. Followed by tablespace ID and the file name. */
    FILE_CREATE = 0x80,
    /** Delete a file. Followed by tablespace ID and the file name.  */
    FILE_DELETE = 0x90,
    /** Rename a file. Followed by tablespace ID and the old file name,
    NUL, and the new file name.  */
    FILE_RENAME = 0xa0,
    /** Modify a file. Followed by tablespace ID and the file name. */
    FILE_MODIFY = 0xb0,
    /** End-of-checkpoint marker, at the end of a mini-transaction.
    Followed by 2 NUL bytes of page identifier and 8 bytes of LSN;
    @see SIZE_OF_FILE_CHECKPOINT.
    When all bytes are NUL, this is a dummy padding record. */
    FILE_CHECKPOINT = 0xf0,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MtrOperation {
    FreePage = mrec_type_t::FREE_PAGE as u8,
    InitPage = mrec_type_t::INIT_PAGE as u8,
    Extended = mrec_type_t::EXTENDED as u8,
    Write = mrec_type_t::WRITE as u8,
    Memset = mrec_type_t::MEMSET as u8,
    Memmove = mrec_type_t::MEMMOVE as u8,
    Reserved = mrec_type_t::RESERVED as u8,
    Option = mrec_type_t::OPTION as u8,
    FileCreate = mfile_type_t::FILE_CREATE as u8,
    FileDelete = mfile_type_t::FILE_DELETE as u8,
    FileRename = mfile_type_t::FILE_RENAME as u8,
    FileModify = mfile_type_t::FILE_MODIFY as u8,
    FileCheckpoint = mfile_type_t::FILE_CHECKPOINT as u8,
}

impl From<u8> for MtrOperation {
    fn from(value: u8) -> Self {
        match value {
            x if x == mrec_type_t::FREE_PAGE as u8 => MtrOperation::FreePage,
            x if x == mrec_type_t::INIT_PAGE as u8 => MtrOperation::InitPage,
            x if x == mrec_type_t::EXTENDED as u8 => MtrOperation::Extended,
            x if x == mrec_type_t::WRITE as u8 => MtrOperation::Write,
            x if x == mrec_type_t::MEMSET as u8 => MtrOperation::Memset,
            x if x == mrec_type_t::MEMMOVE as u8 => MtrOperation::Memmove,
            x if x == mrec_type_t::RESERVED as u8 => MtrOperation::Reserved,
            x if x == mrec_type_t::OPTION as u8 => MtrOperation::Option,
            x if x == mfile_type_t::FILE_CREATE as u8 => MtrOperation::FileCreate,
            x if x == mfile_type_t::FILE_DELETE as u8 => MtrOperation::FileDelete,
            x if x == mfile_type_t::FILE_RENAME as u8 => MtrOperation::FileRename,
            x if x == mfile_type_t::FILE_MODIFY as u8 => MtrOperation::FileModify,
            x if x == mfile_type_t::FILE_CHECKPOINT as u8 => MtrOperation::FileCheckpoint,
            _ => panic!("Unknown operation type: {value}"),
        }
    }
}
