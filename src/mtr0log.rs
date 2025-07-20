use std::io::{Error, ErrorKind, Read, Result, Write};

use byteorder::{ReadBytesExt, WriteBytesExt};

/// The minimum 2-byte integer (0b10xxxxxx xxxxxxxx)
pub const MIN_2BYTE: u32 = 1 << 7;
/// The minimum 3-byte integer (0b110xxxxx xxxxxxxx xxxxxxxx)
pub const MIN_3BYTE: u32 = MIN_2BYTE + (1 << 14);
/// The minimum 4-byte integer (0b1110xxxx xxxxxxxx xxxxxxxx xxxxxxxx)
pub const MIN_4BYTE: u32 = MIN_3BYTE + (1 << 21);
/// Minimum 5-byte integer (0b11110000 xxxxxxxx xxxxxxxx xxxxxxxx xxxxxxxx)
pub const MIN_5BYTE: u32 = MIN_4BYTE + (1 << 28);

/// Error from mlog_decode_varint()
pub const MLOG_DECODE_ERROR: u32 = !0u32;

/// Decode the length of a variable-length encoded integer.
/// @param first  first byte of the encoded integer
/// @return the length, in bytes
pub fn mlog_decode_varint_length(mut byte: u8) -> u8 {
    let mut len = 1u8;
    while byte & 0x80 > 0 {
        len += 1;
        byte <<= 1;
    }
    len
}

/// Decode an integer in a redo log record.
/// @param log    redo log record buffer
/// @return the decoded integer
/// @retval MLOG_DECODE_ERROR ([`std::io::ErrorKind::InvalidData`]) on error
///
/// Bits 3..0 indicate the redo log record length, excluding the first
/// byte, but including additional length bytes and any other bytes,
/// such as the optional tablespace identifier and page number.
/// Values 1..15 represent lengths of 1 to 15 bytes. The special value 0
/// indicates that 1 to 3 length bytes will follow to encode the remaining
/// length that exceeds 16 bytes.
///
/// Additional length bytes if length>16: 0 to 3 bytes
/// 0xxxxxxx                   for 0 to 127 (total: 16 to 143 bytes)
/// 10xxxxxx xxxxxxxx          for 128 to 16511 (total: 144 to 16527)
/// 110xxxxx xxxxxxxx xxxxxxxx for 16512 to 2113663 (total: 16528 to 2113679)
/// 111xxxxx                   reserved (corrupted record, and file!)
pub fn mlog_decode_varint(mut buf: impl Read) -> Result<u32> {
    let b0 = buf.read_u8()? as u32;

    if b0 < MIN_2BYTE {
        return Ok(b0);
    }

    if b0 < 0xc0 {
        let b1 = buf.read_u8()? as u32;
        return Ok(MIN_2BYTE + ((b0 & !0x80) << 8 | b1));
    }

    if b0 < 0xe0 {
        let b1 = buf.read_u8()? as u32;
        let b2 = buf.read_u8()? as u32;
        return Ok(MIN_3BYTE + ((b0 & !0xc0) << 16 | b1 << 8 | b2));
    }

    if b0 < 0xf0 {
        let b1 = buf.read_u8()? as u32;
        let b2 = buf.read_u8()? as u32;
        let b3 = buf.read_u8()? as u32;
        return Ok(MIN_4BYTE + ((b0 & !0xe0) << 24 | b1 << 16 | b2 << 8 | b3));
    }

    if b0 == 0xf0 {
        let mut b4 = [0u8; 4];
        buf.read_exact(&mut b4)?;
        let b0 = (b4[0] as u32) << 24 | (b4[1] as u32) << 16 | (b4[2] as u32) << 8 | b4[3] as u32;
        if b0 <= !MIN_5BYTE {
            return Ok(MIN_5BYTE + b0);
        }
    }

    Err(Error::new(
        ErrorKind::InvalidData,
        "can't decode mlog varint",
    ))
}

/// Encode an integer in a redo log record.
/// @param log  redo log record buffer
/// @param i    the integer to encode
/// @return end of the encoded integer
pub fn mlog_encode_varint(mut w: impl Write, mut i: u32) -> Result<()> {
    if i < MIN_2BYTE {
        // nothing
    } else if i < MIN_3BYTE {
        i -= MIN_2BYTE;
        // static_assert(MIN_3BYTE - MIN_2BYTE == 1 << 14, "compatibility");
        w.write_u8(0x80 | (i >> 8) as u8)?;
    } else if i < MIN_4BYTE {
        i -= MIN_3BYTE;
        // static_assert(MIN_4BYTE - MIN_3BYTE == 1 << 21, "compatibility");
        w.write_u8(0xc0 | (i >> 16) as u8)?;
        w.write_u8((i >> 8) as u8)?;
    } else if i < MIN_5BYTE {
        i -= MIN_4BYTE;
        // static_assert(MIN_5BYTE - MIN_4BYTE == 1 << 28, "compatibility");
        w.write_u8(0xe0 | (i >> 24) as u8)?;
        w.write_u8((i >> 16) as u8)?;
        w.write_u8((i >> 8) as u8)?;
    } else {
        assert!(i < MLOG_DECODE_ERROR);
        i -= MIN_5BYTE;
        w.write_u8(0xf0)?;
        w.write_u8((i >> 24) as u8)?;
        w.write_u8((i >> 16) as u8)?;
        w.write_u8((i >> 8) as u8)?;
    }

    w.write_u8(i as u8)
}

#[cfg(test)]
mod test {
    use super::{mlog_decode_varint, mlog_encode_varint};

    #[test]
    fn test_varint() {
        let nums: [u32; 4] = [0x01, 0x1234, 0x123456, 0x12345678];
        for num in nums {
            let mut buf = Vec::<u8>::new();
            mlog_encode_varint(&mut buf, num).unwrap();
            assert_eq!(
                mlog_decode_varint(buf.as_slice()).unwrap(),
                num,
                "buf: {buf:#x?}"
            );
        }
    }
}
