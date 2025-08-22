use crate::fil0fil;
use crate::mach;
use crate::univ;
use crate::ut0byte;

/// Get the start of a page frame.
///
/// # Arguments
/// * `ptr` - pointer within a page frame (as usize)
/// * `page_size` - size of the page
///
/// # Returns
/// Start of the page frame (as usize)
#[inline]
pub const fn page_align(ptr: usize, page_size: usize) -> usize {
    let ptr0 = ut0byte::ut_align_down(ptr, page_size);
    debug_assert!(ptr0 % univ::UNIV_PAGE_SIZE_MIN as usize == 0);
    ptr0
}

/// Gets the byte offset within a page frame.
///
/// # Arguments
/// * `ptr` - pointer within a page frame (as usize)
/// * `page_size` - size of the page
///
/// # Returns
/// Offset from the start of the page (as u16)
#[inline]
pub const fn page_offset(ptr: usize, page_size: usize) -> u16 {
    ut0byte::ut_align_offset(ptr, page_size) as u16
}

/// Gets the page number.
///
/// # Arguments
/// * `buf` - slice representing the tablespace
/// * `ptr` - pointer to the page frame (as usize)
/// * `page_size` - size of the page
///
/// # Returns
/// Page number as u32
#[inline]
pub fn page_get_page_no(buf: &[u8], ptr: usize, page_size: usize) -> u32 {
    debug_assert!(ptr == page_align(ptr, page_size));
    mach::mach_read_from_4(&buf[ptr + fil0fil::FIL_PAGE_OFFSET as usize..])
}
