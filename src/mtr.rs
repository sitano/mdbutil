use std::io::{Error, ErrorKind, Result};

use crate::mtr0log::{mlog_decode_varint, mlog_decode_varint_length};
use crate::mtr0types::mfile_type_t::FILE_CHECKPOINT;
use crate::ring::RingReader;

/// MTR termination marker.
/// 0x0 or 0x1 are termination markers.
pub const MTR_END_MARKER: u8 = 1u8;

/// Maximum guaranteed size of a mini-transaction.
pub const MTR_SIZE_MAX: u32 = 1u32 << 20;

/// Space id of the transaction system page (the system tablespace).
pub const TRX_SYS_SPACE: u32 = 0;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Mtr {
    /// total mtr length including 1st byte
    len: u32,
    /// tablespace id
    space_id: u32,
    page_no: u32,
    op: u8,
    /// checksum
    checksum: u32,
}

#[allow(clippy::len_without_is_empty)]
impl Mtr {
    pub fn parse_next(r: &mut RingReader) -> Result<Self> {
        peek_not_end_marker(r)?;

        let mtr_start = r.clone();
        let len = Self::parse_len_byte(r)?;

        // TODO: if (*l != log_sys.get_sequence_bit((l - begin) + lsn))
        //   return GOT_EOF;

        // body length = 1st byte + payload length.
        //         and 1 byte termination marker,
        //         and 4 crc32c checksum.
        let real_crc = mtr_start.crc32c((len + 1) as usize);
        r.advance(1); // past termination marker.

        // TODO: encyption, crc iv 8

        let expected_crc = r.read_4()?; // read block crc.

        if real_crc != expected_crc {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "mtr at pos={pos} (0x{pos_hex:x}) len={len} checksum is invalid, expected {expected_crc:#x}, real {real_crc:#x}",
                    pos = mtr_start.pos(),
                    pos_hex = mtr_start.pos(),
                ),
            ));
        }

        // TODO: for parsing loop

        let mut l = mtr_start.clone();
        let recs = l.clone();
        l.advance(1);

        let b = recs.peek_1()?;

        // move past varint length.
        let mut rlen = (b & 0xf) as u32;
        if rlen > 0 {
            let lenlen = mlog_decode_varint_length(l.peek_1()?);
            // TODO: let addlen = mlog_decode_varint(l.block(lenlen))?;
            // TODO: rlen = addlen + 15 - lenlen as u32;
            l.advance(lenlen as usize);
        }

        // TODO: if ((b & 0x80) && got_page_op) {}

        let space_id_len = mlog_decode_varint_length(l.peek_1()?);
        let space_id = mlog_decode_varint(&mut l)?;
        // l.advance()
        rlen -= space_id_len as u32;

        let page_no_len = mlog_decode_varint_length(l.peek_1()?);
        let page_no = mlog_decode_varint(&mut l)?;
        // l.advance()
        rlen -= page_no_len as u32;

        let mut mtr_op = 0;
        let got_page_op = b & 0x80 == 0;

        if got_page_op {
            // page op
            mtr_op = b & 0x70;
        } else if rlen > 0 {
            // file op
            mtr_op = b & 0xf0;

            if mtr_op == FILE_CHECKPOINT as u8 {
                let lsn = l.read_8()?;
                println!("FILE_CHECKPOINT LSN: {lsn}");
            }
        } else if b == FILE_CHECKPOINT as u8 + 2 && space_id == 0 && page_no == 0 {
            // nothing
        } else {
            todo!("malformed");
        }

        // TODO: l+= log_sys.is_encrypted() ? 4U + 8U : 4U;

        Ok(Mtr {
            len,
            space_id,
            page_no,
            op: mtr_op,
            checksum: real_crc,
        })
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
            0x1f, 0xa3, 0x52, 0x97, // checksum
        ];
        let buf = &storage;
        let mut r0 = RingReader::new(buf);
        let mtr = Mtr::parse_next(&mut r0).unwrap();
        assert_eq!(mtr.len, 10, "len");
    }
}
