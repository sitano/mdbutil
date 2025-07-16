use std::io::{Error, ErrorKind, Result};

use crate::mtr0log::mlog_decode_varint;
use crate::ring::RingReader;

/// MTR termination marker.
/// 0x0 or 0x1 are termination markers.
pub const MTR_END_MARKER: u8 = 1u8;

/// Maximum guaranteed size of a mini-transaction.
pub const MTR_SIZE_MAX: u32 = 1u32 << 20;

#[derive(Debug)]
pub struct Mtr {
    /// total mtr length including 1st byte.
    len: u32,
}

#[allow(clippy::len_without_is_empty)]
impl Mtr {
    pub fn parse_next(r: &mut RingReader) -> Result<Self> {
        peek_not_end_marker(r)?;

        let _mtr_start = r.clone();
        let len = Self::parse_len_byte(r)?;

        Ok(Mtr { len })
    }

    pub fn parse_len_byte(r: &mut RingReader) -> Result<u32> {
        let mut total_len = 0u32;

        loop {
            if total_len >= MTR_SIZE_MAX {
                return Err(Error::from(ErrorKind::NotFound));
            }

            if peek_not_end_marker(r).is_err() {
                // EOM found.
                break;
            }

            let mut rlen = (r.read_1()? & 0xf) as u32;
            if rlen == 0 {
                // unused: rlen = mlog_decode_varint_length(r.peek_1()?) as u32;
                let addlen = mlog_decode_varint(r.clone())?;
                if total_len >= MTR_SIZE_MAX {
                    return Err(Error::from(ErrorKind::NotFound));
                }
                rlen = addlen + 15;
            }

            total_len += rlen;

            r.advance(rlen as usize);
        }

        Ok(total_len)
    }

    pub fn len(&self) -> u32 {
        self.len
    }
}

/// test for EOF. tests if reader points at termination byte marker.
pub fn peek_not_end_marker(r: &mut RingReader) -> Result<()> {
    // 0x0 or 0x1 are termination markers.
    if r.peek_1()? <= MTR_END_MARKER {
        // EOF
        return Err(Error::from(ErrorKind::NotFound));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::Mtr;
    use crate::ring::RingReader;

    #[test]
    fn test_mtr_short_len() {
        let storage = [
            0xfa, // FILE_CHECKPOINT + len 10 bytes (+1 1st byte + 1 termination marker)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xde, 0x3d, //  whatever it is
            0x01, // marker
        ];
        let buf = &storage;
        let mut r0 = RingReader::new(buf);
        let mtr = Mtr::parse_next(&mut r0).unwrap();
        assert_eq!(mtr.len, 10, "len");
    }
}
