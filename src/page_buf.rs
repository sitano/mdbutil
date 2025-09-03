use std::{
    fmt::{Debug, Display},
    io::{Read, Result},
    ops::{Index, RangeFrom, RangeTo},
};

use crc32c::crc32c;

use crate::{Lsn, buf0buf, fil0fil, fsp0types, fut0lst, mach, trx0undo};

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

pub fn make_undo_log_page(
    page: &mut [u8],
    space_id: u32,
    page_no: u32,
    page_lsn: Lsn,
    flags: u32,
) -> Result<()> {
    assert!(fil0fil::full_crc32(flags));
    assert!(fsp0types::FSP_FLAGS_GET_POST_ANTELOPE(flags) != 0);
    assert!(fsp0types::FSP_FLAGS_HAS_PAGE_COMPRESSION(flags) == 0);

    if flags != 0x15 {
        // only support general tablespace without encryption and compression.
        // just to be sure we didn't miss anything.
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("Unsupported tablespace flags: {:#x}", flags),
        ));
    }

    let page_size = fil0fil::logical_size(flags);
    assert_eq!(page.len(), page_size);

    page.fill(0);

    make_page_header(
        page,
        space_id,
        page_no,
        fil0fil::FIL_PAGE_UNDO_LOG,
        page_lsn,
        flags,
    )?;
    make_undo_log_page_header(&mut page[trx0undo::TRX_UNDO_PAGE_HDR as usize..])?;
    make_page_footer(page)?;

    Ok(())
}

pub fn make_page_header(
    buf: &mut [u8],
    space_id: u32,
    page_no: u32,
    page_type: u16,
    page_lsn: Lsn,
    flags: u32,
) -> Result<()> {
    assert_eq!(flags, 0x15);

    mach::mach_write_to_4(&mut buf[fil0fil::FIL_PAGE_SPACE_OR_CHKSUM as usize..], 0)?; // 0
    mach::mach_write_to_4(&mut buf[fil0fil::FIL_PAGE_OFFSET as usize..], page_no)?; // 4
    mach::mach_write_to_4(
        &mut buf[fil0fil::FIL_PAGE_PREV as usize..],
        fil0fil::FIL_NULL,
    )?; // 8
    mach::mach_write_to_4(
        &mut buf[fil0fil::FIL_PAGE_NEXT as usize..],
        fil0fil::FIL_NULL,
    )?; // 12
    mach::mach_write_to_8(&mut buf[fil0fil::FIL_PAGE_LSN as usize..], page_lsn)?; // 16
    mach::mach_write_to_2(&mut buf[fil0fil::FIL_PAGE_TYPE as usize..], page_type)?; // 24
    mach::mach_write_to_4(&mut buf[fil0fil::FIL_PAGE_SPACE_ID as usize..], space_id)?; // 34

    // total length 38 bytes

    Ok(())
}

pub fn make_undo_log_page_header(buf: &mut [u8]) -> Result<()> {
    // trx_undo_page_t
    mach::mach_write_to_2(&mut buf[trx0undo::TRX_UNDO_PAGE_TYPE as usize..], 0)?; // 0
    mach::mach_write_to_2(
        &mut buf[trx0undo::TRX_UNDO_PAGE_START as usize..],
        trx0undo::TRX_UNDO_PAGE_HDR_SIZE as u16,
    )?; // 2, 38 + 6 + 2 * 6 = 56
    mach::mach_write_to_2(
        &mut buf[trx0undo::TRX_UNDO_PAGE_FREE as usize..],
        trx0undo::TRX_UNDO_PAGE_HDR_SIZE as u16,
    )?; // 2, 38 + 6 + 2 * 6 = 56

    let mut empty_page_node = fut0lst::flst_node_t::default();
    empty_page_node.read_exact(
        &mut buf[trx0undo::TRX_UNDO_PAGE_NODE as usize
            ..trx0undo::TRX_UNDO_PAGE_NODE as usize + fut0lst::FLST_NODE_SIZE as usize],
    )?;

    Ok(())
}

pub fn make_page_footer(page_buf: &mut [u8]) -> Result<()> {
    let page_size = page_buf.len();

    assert!(page_size.is_power_of_two());

    let end_lsn_offset = page_size - fil0fil::FIL_PAGE_FCRC32_END_LSN as usize;
    let checksum_offset = page_size - fil0fil::FIL_PAGE_FCRC32_CHECKSUM as usize;

    let page_lsn = mach::mach_read_from_8(&page_buf[fil0fil::FIL_PAGE_LSN as usize..]) as u32;
    mach::mach_write_to_4(&mut page_buf[end_lsn_offset..], page_lsn)?;

    let crc32 = crc32c(&page_buf[..checksum_offset]);
    mach::mach_write_to_4(&mut page_buf[checksum_offset..], crc32)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::PageBuf;
    use crate::fil0fil;

    #[test]
    pub fn make_undo_log_page_test() {
        let flags = 0x15u32; // general full crc32 tablespace without encryption and compression
        let page_size = 16 * 1024;
        let space_id = 1;
        let page_no = 50;
        let page_lsn = 789;

        let mut page = vec![0u8; page_size];

        super::make_undo_log_page(&mut page, space_id, page_no, page_lsn, flags).unwrap();

        let page = PageBuf::new(0x15, &page);

        assert_eq!(page.space_id, space_id);
        assert_eq!(page.page_no, page_no);
        assert_eq!(page.page_lsn, page_lsn);
        assert_eq!(page.page_type, fil0fil::FIL_PAGE_UNDO_LOG);
        assert_eq!(page.head_checksum, 0);
        assert_eq!(page.foot_lsn, page_lsn as u32);

        page.corrupted(Some(789)).unwrap();
    }
}
