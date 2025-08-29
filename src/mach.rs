// Functions related to encoding.
use std::io::{Result, Write};

use byteorder::{BigEndian, ByteOrder};

// MariaDB uses big-endian byte order for its Mach-O files.
// The most significant byte is at the lowest address.
type E = BigEndian;

pub fn mach_read_from_2(buf: &[u8]) -> u16 {
    E::read_u16(buf)
}

pub fn mach_read_from_4(buf: &[u8]) -> u32 {
    E::read_u32(buf)
}

pub fn mach_read_from_8(buf: &[u8]) -> u64 {
    E::read_u64(buf)
}

pub fn mach_write_to_4(mut buf: impl Write, value: u32) -> Result<()> {
    buf.write_all(&value.to_be_bytes())
}

pub fn mach_write_to_8(mut buf: impl Write, value: u64) -> Result<()> {
    buf.write_all(&value.to_be_bytes())
}
