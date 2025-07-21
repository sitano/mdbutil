use std::io::{Error, ErrorKind, Result, Write};

use crate::Lsn;
use crate::mach::{mach_write_to_4, mach_write_to_8};
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
    pub len: u32,

    /// tablespace id
    pub space_id: u32,
    pub page_no: u32,

    pub op: u8,

    /// checksum
    pub checksum: u32,

    // payload
    //
    // FILE_CHECKPOINT LSN, if any.
    pub file_checkpoint_lsn: Option<Lsn>,
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
        // mtr also has 1 byte termination marker,
        //          and 4 crc32c checksum.
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
        if rlen == 0 {
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
        let mut file_checkpoint_lsn = None;
        let got_page_op = b & 0x80 == 0;

        if got_page_op {
            // page op
            mtr_op = b & 0x70;
        } else if rlen > 0 {
            // file op
            mtr_op = b & 0xf0;

            if mtr_op == FILE_CHECKPOINT as u8 {
                let lsn = l.read_8()? as Lsn;
                file_checkpoint_lsn = Some(lsn);
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
            file_checkpoint_lsn,
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

    pub fn build_file_checkpoint(mut buf: impl Write, lsn: Lsn) -> Result<()> {
        // 0xfa is FILE_CHECKPOINT + 10b + 1b termination marker + 4b checksum)
        let mut temp = [0u8; 1 + 10 + 1 + 4];
        let mut cursor = std::io::Cursor::new(temp.as_mut_slice());

        cursor.write_all(&[0xfa])?; // FILE_CHECKPOINT + body len 10 bytes
        cursor.write_all(&[0x00, 0x00])?; // tablespace id + page no
        mach_write_to_8(&mut cursor, lsn)?; // checkpoint LSN

        cursor.write_all(&[MTR_END_MARKER])?; // termination marker

        let checksum = crc32c::crc32c(&cursor.get_ref()[..1 + 10]);
        mach_write_to_4(&mut cursor, checksum)?;

        buf.write_all(cursor.get_ref())?;

        Ok(())
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

    use crate::mtr0types::mfile_type_t::FILE_CHECKPOINT;
    use crate::ring::RingReader;

    #[test]
    fn test_mtr_short_len() {
        let storage = [
            0xfa, // FILE_CHECKPOINT + len 10 bytes (+1 1st byte + 1 termination marker)
            0x00, 0x00, // tablespace id + page no
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xde, 0x3d, // checkpoint LSN
            0x01, // marker
            0x1f, 0xa3, 0x52, 0x97, // checksum
        ];
        let buf = &storage;
        let mut r0 = RingReader::new(buf);
        let mtr = Mtr::parse_next(&mut r0).unwrap();
        assert_eq!(mtr.len, 10, "len");
    }

    #[test]
    fn test_build_file_checkpoint() {
        let mut buf = Vec::new();
        let lsn = 0x000000000000de3d;
        Mtr::build_file_checkpoint(&mut buf, lsn).unwrap();

        let mut r0 = RingReader::new(buf.as_slice());
        let mtr = Mtr::parse_next(&mut r0).unwrap();

        assert_eq!(mtr.op, FILE_CHECKPOINT as u8, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
        assert_eq!(mtr.len, 10, "len");
        assert_eq!(mtr.file_checkpoint_lsn, Some(lsn), "file_checkpoint_lsn");
    }
}
