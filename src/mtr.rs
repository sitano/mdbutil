use std::fmt::Display;
use std::io::{Error, ErrorKind, Result, Write};

use crate::{
    Lsn,
    mach::{mach_write_to_4, mach_write_to_8},
    mtr0log::{mlog_decode_varint, mlog_decode_varint_length},
    mtr0types::MtrOperation,
    mtr0types::mfile_type_t::FILE_CHECKPOINT,
    ring::RingReader,
};

/// MTR termination marker.
/// 0x0 or 0x1 are termination markers.
/// Termination marker corresponds to LSN by the means of generation:
///    !(((lsn - header_size) / capacity & 1))
pub const MTR_END_MARKER: u8 = 1u8;

/// Maximum guaranteed size of a mini-transaction.
pub const MTR_SIZE_MAX: u32 = 1u32 << 20;

/// Space id of the transaction system page (the system tablespace).
pub const TRX_SYS_SPACE: u32 = 0;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Mtr {
    /// total mtr length including 1st byte, termination marker and checksum.
    pub len: u32,

    /// tablespace id
    pub space_id: u32,
    pub page_no: u32,

    pub op: MtrOperation,

    // termination marker
    pub marker: u8,
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
        let lsn = mtr_start.pos() as Lsn;
        let _ = Self::find_end_marker(r)?;

        let termination_marker_offset = r.pos() - mtr_start.pos();
        // following is equivalent to r.peek_1()?.
        let termination_byte = (&mtr_start + termination_marker_offset).peek_1()?;
        let termination_lsn = lsn + termination_marker_offset as u64;

        if termination_byte
            != get_sequence_bit(r.header() as u64, r.capacity() as u64, termination_lsn)
        {
            return Err(Error::from(ErrorKind::NotFound));
        }

        // |MTR|MTR|...|^TERMINATION_MARKER|CHECKSUM|.
        let real_crc = mtr_start.crc32c(termination_marker_offset);
        r.advance(1); // past termination marker.

        // TODO: encyption, crc iv 8

        let expected_crc = r.read_4()?; // read block crc.

        if real_crc != expected_crc {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "mtr at pos={pos} (0x{pos_hex:x}) len={len} checksum is invalid, expected \
                     {expected_crc:#x}, real {real_crc:#x}",
                    pos = mtr_start.pos(),
                    pos_hex = mtr_start.pos(),
                    len = termination_marker_offset + 1 + 4,
                ),
            ));
        }

        // TODO: for parsing loop

        // println!(
        //     "mtr at pos={pos} (0x{pos_hex:x}) len={len} checksum {real_crc:#x}",
        //     pos = mtr_start.pos(),
        //     pos_hex = mtr_start.pos(),
        //     len = termination_marker_offset + 1 + 4,
        // );
        // let mut buf = vec![0u8; termination_marker_offset + 1 + 4];
        // mtr_start.block(buf.as_mut_slice());
        // println!("mtr: {buf:x?}");

        let mut l = mtr_start.clone();
        let recs = l.clone();
        l.advance(1);

        let b = recs.peek_1()?;

        // move past varint length.
        let mut rlen = (b & 0xf) as u32;
        if rlen == 0 {
            let lenlen = mlog_decode_varint_length(l.peek_1()?);
            let addlen = mlog_decode_varint(&mut l)?;
            rlen = addlen + 15 - lenlen as u32;
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
            len: termination_marker_offset as u32 + 1 + 4,
            space_id,
            page_no,
            op: mtr_op.into(),
            marker: termination_byte,
            checksum: real_crc,
            file_checkpoint_lsn,
        })
    }

    /// Looks through the MTR chain end finds the end marker.
    /// Where the chain is |MTR|MTR|...|^TERMINATION_MARKER|CHECKSUM|.
    /// Header byte, termination marker and checksum are not included
    /// in the payload length.
    pub fn find_end_marker(r: &mut RingReader) -> Result<u32> {
        let mut payload_len = 0u32;

        loop {
            if payload_len >= MTR_SIZE_MAX {
                return Err(Error::from(ErrorKind::NotFound));
            }

            if peek_not_end_marker(r).is_err() {
                // EOM found.
                break;
            }

            let mut rlen = (r.read_1()? & 0xf) as u32;
            if rlen == 0 {
                let addlen = mlog_decode_varint(r.clone())?;
                if payload_len >= MTR_SIZE_MAX {
                    return Err(Error::from(ErrorKind::NotFound));
                }
                rlen = addlen + 15;
            }

            payload_len += rlen;

            r.advance(rlen as usize);
        }

        Ok(payload_len)
    }

    pub fn len(&self) -> u32 {
        self.len
    }

    pub fn build_file_checkpoint(
        mut buf: impl Write,
        header: u64,
        capacity: u64,
        lsn: Lsn,
    ) -> Result<()> {
        // 0xfa is FILE_CHECKPOINT + 10b + 1b termination marker + 4b checksum)
        let mut temp = [0u8; 1 + 10 + 1 + 4];
        let mut cursor = std::io::Cursor::new(temp.as_mut_slice());

        cursor.write_all(&[0xfa])?; // FILE_CHECKPOINT + body len 10 bytes
        cursor.write_all(&[0x00, 0x00])?; // tablespace id + page no
        mach_write_to_8(&mut cursor, lsn)?; // checkpoint LSN

        let termination_marker = get_sequence_bit(header, capacity, lsn + 1 + 2 + 8);
        cursor.write_all(&[termination_marker])?;

        let checksum = crc32c::crc32c(&cursor.get_ref()[..1 + 10]);
        mach_write_to_4(&mut cursor, checksum)?;

        buf.write_all(cursor.get_ref())?;

        Ok(())
    }
}

impl Display for Mtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mtr {{ len: {}, space_id: {}, page_no: {}, op: {:?} }}",
            self.len, self.space_id, self.page_no, self.op
        )
    }
}

/// Determine the sequence bit at a log sequence number.
/// The sequence bit is used to determine whether the log record
/// corresponds to the current generation (wrap) of the redo log.
/// Capacity is the capacity of the ring buffer in bytes (file size - header).
pub fn get_sequence_bit(header_size: u64, capacity: u64, lsn: Lsn) -> u8 {
    if (((lsn - header_size) / capacity) & 1) == 0 {
        1
    } else {
        0
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
    use crate::{mtr0types::MtrOperation, ring::RingReader};

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
        assert_eq!(mtr.len, 16, "len");
    }

    #[test]
    fn test_build_file_checkpoint_marker_1() {
        let mut buf = Vec::new();
        let lsn = 0x000000000000de3d;
        let hdr_size = 0;
        let fake_capacity = 0xffff;
        let marker = super::get_sequence_bit(hdr_size, fake_capacity, lsn);
        Mtr::build_file_checkpoint(&mut buf, hdr_size, fake_capacity, lsn).unwrap();

        let r0 = RingReader::new(buf.as_slice());
        let mtr = Mtr::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
        assert_eq!(mtr.len, 16, "len");
        assert_eq!(mtr.file_checkpoint_lsn, Some(lsn), "file_checkpoint_lsn");

        assert_eq!(marker, 1);
        assert_eq!(
            (&r0 + (buf.len() - 4 - 1)).peek_1().unwrap(),
            marker,
            "termination marker"
        );
    }

    #[test]
    fn test_build_file_checkpoint_marker_0() {
        let mut buf = Vec::new();
        let lsn = 0x0000000000000030;
        let hdr_size = 0;
        let fake_capacity = 0x10;
        let marker = super::get_sequence_bit(hdr_size, fake_capacity, lsn);
        Mtr::build_file_checkpoint(&mut buf, hdr_size, fake_capacity, lsn).unwrap();

        let r0 = RingReader::buf_at(buf.as_slice(), hdr_size as usize, lsn as usize);
        let mtr = Mtr::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
        assert_eq!(mtr.len, 16, "len");
        assert_eq!(mtr.file_checkpoint_lsn, Some(lsn), "file_checkpoint_lsn");

        assert_eq!(marker, 0);
        assert_eq!(
            (&r0 + (buf.len() - 4 - 1)).peek_1().unwrap(),
            marker,
            "termination marker"
        );
    }

    #[test]
    fn test_parse_next_respects_old_gen() {
        let mut buf = Vec::new();
        // 0x30 / 0x10 = 0x3 & 1 = 1, so the sequence bit is 0.
        let lsn = 0x0000000000000030;
        let hdr_size = 0;
        let fake_capacity = 0x10;
        let marker = super::get_sequence_bit(hdr_size, fake_capacity, lsn);
        Mtr::build_file_checkpoint(&mut buf, hdr_size, fake_capacity, lsn).unwrap();

        let r0 = RingReader::buf_at(buf.as_slice(), hdr_size as usize, lsn as usize);
        let mtr = Mtr::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
        assert_eq!(mtr.len, 16, "len");
        assert_eq!(mtr.file_checkpoint_lsn, Some(lsn), "file_checkpoint_lsn");

        assert_eq!(marker, 0);
        assert_eq!(
            (&r0 + (buf.len() - 4 - 1)).peek_1().unwrap(),
            marker,
            "termination marker"
        );
    }

    #[test]
    fn test_parse_next_can_parse_wrap_with_invalid_marker() {
        let mut buf0 = Vec::new();
        // 0x30 / 0x10 = 0x3 & 1 = 1, so the sequence bit is 0.
        let lsn = 0x000000000000003a;
        let hdr_size = 0;
        let fake_capacity = 0x10usize;
        Mtr::build_file_checkpoint(&mut buf0, hdr_size, fake_capacity as u64, lsn).unwrap();

        let mut buf = vec![0u8; fake_capacity];
        let offset = lsn as usize % fake_capacity;
        buf[..offset].copy_from_slice(&buf0[..offset]);
        buf[offset..].copy_from_slice(&buf0[offset..]);

        let r0 = RingReader::buf_at(buf.as_slice(), hdr_size as usize, lsn as usize);
        assert!(Mtr::parse_next(&mut r0.clone()).is_err());
    }

    #[test]
    fn test_parse_next_can_parse_wrap_with_valid_marker() {
        let mut buf0 = Vec::new();
        // 0x30 / 0x10 = 0x3 & 1 = 1, so the sequence bit is 0.
        let lsn = 0x000000000000002a;
        let hdr_size = 0;
        let fake_capacity = 0x10usize;
        Mtr::build_file_checkpoint(&mut buf0, hdr_size, fake_capacity as u64, lsn).unwrap();

        let mut buf = vec![0u8; fake_capacity];
        let offset = lsn as usize % fake_capacity;
        buf[..offset].copy_from_slice(&buf0[..offset]);
        buf[offset..].copy_from_slice(&buf0[offset..]);

        let r0 = RingReader::buf_at(buf.as_slice(), hdr_size as usize, lsn as usize);
        assert!(Mtr::parse_next(&mut r0.clone()).is_err());
    }
}
