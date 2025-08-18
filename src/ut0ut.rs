/// Determine how many bytes (groups of 8 bits) are needed to
/// store the given number of bits.
///
/// # Arguments
/// * `bits` - Number of bits
///
/// # Returns
/// Number of bytes (octets) needed to represent `bits`
#[inline]
#[allow(non_snake_case)]
pub const fn UT_BITS_IN_BYTES(bits: u32) -> u32 {
    (bits + 7) >> 3
}

/// Determines if a number is zero or a power of two.
///
/// # Arguments
/// * `n` - Number
///
/// # Returns
/// `true` if `n` is zero or a power of two; `false` otherwise
#[inline]
#[allow(non_snake_case)]
pub const fn UT_IS_2POW(n: u32) -> bool {
    (n & (n.wrapping_sub(1))) == 0
}
