use std::{
    fmt::Display,
    io::{Error, ErrorKind, Result, Write},
};

use crate::{
    Lsn,
    mach::{mach_write_to_4, mach_write_to_8},
    mtr0log::{mlog_decode_varint, mlog_decode_varint_length},
    mtr0types::{
        MtrOperation,
        mfile_type_t::FILE_CHECKPOINT,
        mrec_type_t::{INIT_PAGE, MEMSET, RESERVED},
    },
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtrChain {
    pub lsn: Lsn,
    /// total mtr length including 1st byte, termination marker and checksum.
    pub len: u32,
    pub checksum: u32,
    pub mtr: Vec<Mtr>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mtr {
    /// tablespace id
    pub space_id: u32,
    pub page_no: u32,

    pub op: MtrOperation,

    // FILE_CHECKPOINT LSN, if any.
    pub file_checkpoint_lsn: Option<Lsn>,

    // termination marker
    pub marker: u8,
}

#[allow(clippy::len_without_is_empty)]
impl MtrChain {
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
        let real_crc = mtr_start.crc32c(termination_marker_offset)?;
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

        // println!(
        //     "mtr at pos={pos} (0x{pos_hex:x}) len={len} checksum {real_crc:#x}",
        //     pos = mtr_start.pos(),
        //     pos_hex = mtr_start.pos(),
        //     len = termination_marker_offset + 1 + 4,
        // );

        // Parse MTR chain.
        let mut chain = MtrChain {
            lsn,
            len: termination_marker_offset as u32 + 1 + 4,
            checksum: real_crc,
            mtr: Vec::new(),
        };

        let mut l = mtr_start.clone();
        let mut rlen: u32;
        // let mut last_offset = 0u32;
        let mut got_page_op = false;
        let mut space_id = 0u32;
        let mut page_no = 0u32;

        loop {
            // println!(
            //     "mtr at pos={pos} (0x{pos_hex:x}) len={len} checksum {real_crc:#x}",
            //     pos = mtr_start.pos(),
            //     pos_hex = mtr_start.pos(),
            //     len = termination_marker_offset + 1 + 4,
            // );
            // let mut buf = vec![0u8; termination_marker_offset + 1 + 4];
            // mtr_start.block(buf.as_mut_slice());
            // println!("mtr: {buf:x?}");

            let recs = l.clone();
            l.advance(1);

            let b = recs.peek_1()?;

            if b & 0x70 != RESERVED as u8 {
                // fine
            } else {
                eprintln!("InnoDB: Ignoring unknown log record at LSN {}", l.pos());
            }

            if peek_not_end_marker(&recs).is_err() {
                // EOM found.
                break;
            }

            // move past varint length.
            rlen = (b & 0xf) as u32;
            if rlen == 0 {
                let lenlen = mlog_decode_varint_length(l.peek_1()?);
                let addlen = mlog_decode_varint(&mut l)?;
                rlen = addlen + 15 - lenlen as u32;
            }

            // println!(
            //     "rlen: {rlen}, b = {b:#x}, op = {op2:?}, lsn = {:x}, pos = {:x}",
            //     l.pos(),
            //     l.pos_to_offset(l.pos())
            // );

            // If MTR is not a page op over the same page read the space id and page no.
            // not ((b & 0x80 != 0) && got_page_op)
            if !got_page_op || b & 0x80 == 0 {
                let space_id_len = mlog_decode_varint_length(l.peek_1()?);
                space_id = mlog_decode_varint(&mut l)?;
                if rlen < space_id_len as u32 {
                    eprintln!(
                        "InnoDB: Ignoring malformed log record at LSN {}: space_id_len {} < rlen \
                         {}",
                        l.pos(),
                        space_id_len,
                        rlen
                    );
                    break;
                }
                rlen -= space_id_len as u32;

                let page_no_len = mlog_decode_varint_length(l.peek_1()?);
                page_no = mlog_decode_varint(&mut l)?;
                if rlen < page_no_len as u32 {
                    eprintln!(
                        "InnoDB: Ignoring malformed log record at LSN {}: page_no_len {} < rlen {}",
                        l.pos(),
                        page_no_len,
                        rlen
                    );
                    break;
                }
                rlen -= page_no_len as u32;

                got_page_op = b & 0x80 == 0;
            } else {
                // TODO: verify the same page op precond.
                // This record is for the same page as the previous one.
                if (b & 0x70) <= INIT_PAGE as u8 {
                    // record is corrupted.
                    // FREE_PAGE,INIT_PAGE cannot be with same_page flag.
                    eprintln!("InnoDB: Ignoring malformed log record at LSN {}", l.pos());
                    // the next record must not be same_page.
                    continue;
                }
                // DBUG_PRINT("ib_log",
                //            ("scan " LSN_PF ": rec %x len %zu page %u:%u",
                //             lsn, b, l - recs + rlen, space_id, page_no));
            }

            let mut mtr_op = 0;
            let mut file_checkpoint_lsn = None;

            if got_page_op {
                // page op
                mtr_op = b & 0x70;

                if mtr_op == MEMSET as u8 {
                    let olen = mlog_decode_varint_length(l.peek_1()?);
                    let _offset = mlog_decode_varint(&mut l)?;

                    rlen -= olen as u32;
                }
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

            let op = match MtrOperation::try_from(mtr_op)
                .map_err(|_| Error::from(ErrorKind::InvalidData))
            {
                Ok(op) => op,
                Err(_) => {
                    eprintln!(
                        "InnoDB: Ignoring malformed log record at LSN {}: invalid mtr op {}. \
                         Probably the log is corrupted.",
                        l.pos(),
                        mtr_op
                    );

                    if l.pos() >= mtr_start.pos() + chain.len() as usize {
                        eprintln!(
                            "InnoDB: We are behind the end of the MTR chain at LSN {} >= {}+{}. \
                             Stopping here.",
                            l.pos(),
                            mtr_start.pos(),
                            chain.len()
                        );

                        break;
                    }

                    continue;
                }
            };

            chain.mtr.push(Mtr {
                space_id,
                page_no,
                op,
                file_checkpoint_lsn,
                marker: termination_byte,
            });

            l.advance(rlen as usize);
        }

        Ok(chain)
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

            if !r.advance(rlen as usize) {
                // if ring buffer pos overflow is not supported we don't want it.
                return Err(Error::from(ErrorKind::NotFound));
            }
        }

        Ok(payload_len)
    }

    pub fn len(&self) -> u32 {
        self.len
    }
}

impl Mtr {
    pub fn build_file_checkpoint(
        mut buf: impl Write,
        header: u64,
        capacity: u64,
        lsn: Lsn,
    ) -> Result<()> {
        if lsn < header {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                "LSN must be greater than or equal to the header size",
            ));
        }

        // 16 bytes is the record + 0x00 is the last termination marker.
        if lsn >= u64::MAX - 16 {
            return Err(Error::new(
                ErrorKind::UnexpectedEof,
                "LSN is too large to fit in a file checkpoint",
            ));
        }

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

impl Display for MtrChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "MtrChain {{ len: {}, checksum: {}, mtr: {:?} }}",
            self.len, self.checksum, self.mtr
        )
    }
}

impl Display for Mtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Mtr {{ space_id: {}, page_no: {}, op: {:?} }}",
            self.space_id, self.page_no, self.op
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
pub fn peek_not_end_marker(r: &RingReader) -> Result<()> {
    // 0x0 or 0x1 are termination markers.
    if r.peek_1()? <= MTR_END_MARKER {
        // EOF
        return Err(Error::from(ErrorKind::NotFound));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::{Mtr, MtrChain};
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
        let chain = MtrChain::parse_next(&mut r0).unwrap();
        assert_eq!(chain.len, 16, "len");
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
        let chain = MtrChain::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(chain.len, 16, "len");

        let mtr = &chain.mtr[0];
        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
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
        let chain = MtrChain::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(chain.len, 16, "len");

        let mtr = &chain.mtr[0];
        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
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
        let chain = MtrChain::parse_next(&mut r0.clone()).unwrap();

        assert_eq!(chain.len, 16, "len");

        let mtr = &chain.mtr[0];
        assert_eq!(mtr.op, MtrOperation::FileCheckpoint, "op");
        assert_eq!(mtr.space_id, 0, "space_id");
        assert_eq!(mtr.page_no, 0, "page_no");
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
        assert!(MtrChain::parse_next(&mut r0.clone()).is_err());
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
        assert!(MtrChain::parse_next(&mut r0.clone()).is_err());
    }

    #[test]
    fn test_parse_mtr_chain() {
        let buf = vec![
            // MTR Chain count=2, len=123, lsn=163
            //   1: Mtr { space_id: 3, page_no: 45, op: Extended }
            //   2: Mtr { space_id: 3, page_no: 45, op: Option }
            0x20, 0x5e, 0x3, 0x2d, 0x3, 0xd, 0x3, 0xf, 0x20, 0x0, 0x0, 0x0, 0x0, 0x17, 0xc6, 0x0,
            0x0, 0x0, 0x2d, 0x1, 0x78, 0x4, 0x74, 0x65, 0x73, 0x74, 0x1, 0x61, 0x7, 0x50, 0x52,
            0x49, 0x4d, 0x41, 0x52, 0x59, 0xc, 0x6e, 0x5f, 0x64, 0x69, 0x66, 0x66, 0x5f, 0x70,
            0x66, 0x78, 0x30, 0x31, 0x3, 0x6, 0x4, 0x68, 0x84, 0xa2, 0x89, 0x7, 0x8, 0x0, 0x0, 0x0,
            0x0, 0x0, 0x0, 0x0, 0x6, 0x8, 0x8, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x1, 0x0, 0x22,
            0x0, 0x4, 0x74, 0x65, 0x73, 0x74, 0x1, 0x1, 0x61, 0x2, 0x7, 0x50, 0x52, 0x49, 0x4d,
            0x41, 0x52, 0x59, 0x3, 0xc, 0x6e, 0x5f, 0x64, 0x69, 0x66, 0x66, 0x5f, 0x70, 0x66, 0x78,
            0x30, 0x31, 0x77, 0x3, 0x2d, 0x0, 0x80, 0x89, 0x7e, 0x61, 0x0, 0xa8, 0xf3, 0xd8, 0x55,
            // MTR Chain count=1, len=39, lsn=286
            //   1: Mtr { space_id: 0, page_no: 0, op: FileModify }
            0xb0, 0x12, 0x4, 0x0, 0x2e, 0x2f, 0x6d, 0x79, 0x73, 0x71, 0x6c, 0x2f, 0x69, 0x6e, 0x6e,
            0x6f, 0x64, 0x62, 0x5f, 0x74, 0x61, 0x62, 0x6c, 0x65, 0x5f, 0x73, 0x74, 0x61, 0x74,
            0x73, 0x2e, 0x69, 0x62, 0x64, 0x0, 0xff, 0x42, 0xf0, 0x81,
            // Termination marker.
            0x00,
        ];

        let mut r0 = RingReader::buf_at(buf.as_slice(), 0, buf.len());
        let chain = MtrChain::parse_next(&mut r0).unwrap();
        // println!("Parsed MTR chain: {chain:?}");

        assert_eq!(chain.len(), 123, "chain len in bytes");
        assert_eq!(chain.mtr.len(), 2, "chain mtr count");

        let chain = MtrChain::parse_next(&mut r0).unwrap();
        // println!("Parsed MTR chain: {chain:?}");

        assert_eq!(chain.len(), 39, "chain len in bytes");
        assert_eq!(chain.mtr.len(), 1, "chain mtr count");
    }
}
