/// Round down a pointer to the nearest aligned address.
///
/// # Arguments
/// * `ptr` - pointer as usize
/// * `alignment` - a power of 2
///
/// # Returns
/// Aligned pointer as usize
#[inline]
pub const fn ut_align_down(ptr: usize, alignment: usize) -> usize {
    debug_assert!(alignment > 0);
    debug_assert!(alignment.is_power_of_two());
    ptr & !(alignment - 1)
}

/// Compute the offset of a pointer from the nearest aligned address.
///
/// # Arguments
/// * `ptr` - pointer as usize
/// * `alignment` - a power of 2
///
/// # Returns
/// Distance from aligned pointer as usize
#[inline]
pub const fn ut_align_offset(ptr: usize, alignment: usize) -> usize {
    debug_assert!(alignment > 0);
    debug_assert!(alignment.is_power_of_two());
    ptr & (alignment - 1)
}
