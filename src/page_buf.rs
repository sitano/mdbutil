use std::fmt::Debug;
use std::fmt::Display;
use std::io::Result;
use std::ops::{Index, RangeFrom, RangeTo};

use crate::buf0buf;
use crate::fil0fil;
use crate::mach;

use crate::Lsn;

// TODO: support for compression and encryption
#[derive(Clone)]
pub struct PageBuf<'a> {
    pub space_id: u32,
    pub page_no: u32,

    pub prev_page: u32,
    pub next_page: u32,
    pub page_lsn: Lsn,

    /// The contents of this field can only be trusted in the following case: if the page
    /// is an uncompressed B-tree index page, then it is guaranteed that the value is
    /// FIL_PAGE_INDEX. The opposite does not hold.
    ///
    /// In tablespaces created by MySQL/InnoDB 5.1.7 or later, the contents of this field is valid
    /// for all uncompressed pages.
    pub page_type: u16,

    // if flags has no FSP_FLAGS_FCRC32_MASK_MARKER is not a checksum.
    // see buf0buf::buf_page_is_corrupted().
    pub head_checksum: u32,
    pub foot_checksum: u32,
    pub foot_lsn: u32,

    // tablespace flags
    flags: u32,

    buf: &'a [u8],
}

/// 'null' (undefined) page offset in the context of file spaces.
pub const FIL_NULL: u32 = fil0fil::FIL_NULL;

impl<'a> PageBuf<'a> {
    /// Create a new PageBuf from a byte slice.
    /// The slice is expected to be a full page size, including header and footer.
    /// The flags parameter is the tablespace flags.
    pub fn new(flags: u32, buf: &'a [u8]) -> Self {
        // header
        let head_checksum =
            mach::mach_read_from_4(&buf[fil0fil::FIL_PAGE_SPACE_OR_CHKSUM as usize..]); // 0
        let page_no = mach::mach_read_from_4(&buf[fil0fil::FIL_PAGE_OFFSET as usize..]); // 4
        let prev_page = mach::mach_read_from_4(&buf[fil0fil::FIL_PAGE_PREV as usize..]); // 8
        let next_page = mach::mach_read_from_4(&buf[fil0fil::FIL_PAGE_NEXT as usize..]); // 12
        let page_lsn = mach::mach_read_from_8(&buf[fil0fil::FIL_PAGE_LSN as usize..]) as Lsn; // 16
        let page_type = mach::mach_read_from_2(&buf[fil0fil::FIL_PAGE_TYPE as usize..]); // 24
        let space_id = mach::mach_read_from_4(&buf[fil0fil::FIL_PAGE_SPACE_ID as usize..]); // 34

        // footer
        let foot_lsn =
            mach::mach_read_from_4(&buf[(buf.len() - fil0fil::FIL_PAGE_FCRC32_END_LSN as usize)..]);
        let foot_checksum = mach::mach_read_from_4(
            &buf[(buf.len() - fil0fil::FIL_PAGE_FCRC32_CHECKSUM as usize)..],
        );

        Self {
            space_id,
            page_no,
            prev_page,
            next_page,
            page_lsn,
            page_type,
            head_checksum,
            foot_checksum,
            foot_lsn,
            flags,
            buf,
        }
    }

    pub fn space_id(&self) -> u32 {
        self.space_id
    }

    pub fn page_no(&self) -> u32 {
        self.page_no
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn buf(&self) -> &[u8] {
        self.buf
    }

    pub fn page_ptr(&self) -> usize {
        self.page_no as usize * self.buf.len()
    }

    pub fn page_size(&self) -> usize {
        self.buf.len()
    }

    pub fn corrupted(&self, check_lsn: Option<Lsn>) -> Result<()> {
        buf0buf::buf_page_is_corrupted(self, check_lsn)
    }

    pub fn read_4(&self, offset: usize) -> u32 {
        mach::mach_read_from_4(&self.buf[offset..])
    }

    pub fn read_8(&self, offset: usize) -> u64 {
        mach::mach_read_from_8(&self.buf[offset..])
    }
}

impl std::ops::Deref for PageBuf<'_> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.buf
    }
}

impl Index<usize> for PageBuf<'_> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        &self.buf[index]
    }
}

impl Index<RangeFrom<usize>> for PageBuf<'_> {
    type Output = [u8];

    fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
        &self.buf[index]
    }
}

impl Index<RangeTo<usize>> for PageBuf<'_> {
    type Output = [u8];

    fn index(&self, index: RangeTo<usize>) -> &Self::Output {
        &self.buf[index]
    }
}

impl Debug for PageBuf<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PageBuf")
            .field("space_id", &self.space_id)
            .field("page_no", &self.page_no)
            .field("prev_page", &self.prev_page)
            .field("next_page", &self.next_page)
            .field("page_lsn", &self.page_lsn)
            .field("page_type", &self.page_type)
            .field("head_checksum", &self.head_checksum)
            .field("foot_checksum", &self.foot_checksum)
            .field("foot_lsn", &self.foot_lsn)
            .field("flags", &self.flags)
            .finish()
    }
}

impl Display for PageBuf<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("PageBuf");
        s.field("space_id", &self.space_id);
        s.field("page_no", &self.page_no);
        let prev_page = if self.prev_page == FIL_NULL {
            None
        } else {
            Some(self.prev_page)
        };
        let next_page = if self.next_page == FIL_NULL {
            None
        } else {
            Some(self.next_page)
        };
        s.field("prev_page", &prev_page);
        s.field("next_page", &next_page);
        s.field("page_lsn", &self.page_lsn);
        s.field("page_type", &fil0fil::fil_page_type_t::from(self.page_type));
        s.field("checksum", &self.foot_checksum);
        s.finish()
    }
}
