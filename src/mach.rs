// Functions related to encoding.
use byteorder::{BigEndian, ByteOrder};

// MariaDB uses big-endian byte order for its Mach-O files.
// The most significant byte is at the lowest address.
type E = BigEndian;

pub fn mach_read_from_4(buf: &[u8]) -> u32 {
    E::read_u32(buf)
}

pub fn mach_read_from_8(buf: &[u8]) -> u64 {
    E::read_u64(buf)
}
