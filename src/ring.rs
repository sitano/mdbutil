use std::{
    cmp::min,
    io::{Error, ErrorKind, Read, Result},
    ops::Add,
};

use crc32c::crc32c;

use crate::mach;

// TODO: support Write

#[derive(Debug, Clone)]
pub struct RingReader<'a> {
    buf: &'a [u8],
    pos: usize,
    /// The size of the header in the beginning.
    header: usize,
}

impl<'a> RingReader<'a> {
    pub fn new(buf: &'a [u8]) -> RingReader<'a> {
        Self::buf_at(buf, 0, 0)
    }

    /// Creates a new `RingReader` at the given position in the buffer.
    /// Buffer must be at least `hdr` bytes long and includes the header.
    pub fn buf_at(buf: &'a [u8], hdr: usize, pos: usize) -> RingReader<'a> {
        RingReader {
            buf,
            pos,
            header: hdr,
        }
    }

    /// returns the position in the header+ring_buffer for a given pos.
    pub fn pos_to_offset(&self, pos: usize) -> usize {
        pos_to_offset(self.header, self.buf.len() - self.header, pos)
    }

    // TODO: implement wrap
    pub fn block(&self, size: usize) -> &[u8] {
        let start = self.pos_to_offset(self.pos);
        let end = self.pos_to_offset(self.pos + size);
        &self.buf[start..end]
    }

    pub fn crc32c(&self, size: usize) -> u32 {
        crc32c(self.block(size))
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn header(&self) -> usize {
        self.header
    }

    pub fn capacity(&self) -> usize {
        self.buf.len() - self.header
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn ensure(&self, t: usize) -> Result<()> {
        if self.len() < t {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }

        Ok(())
    }

    pub fn advance(&mut self, bytes: usize) {
        self.pos += bytes;
    }

    pub fn peek_1(&self) -> Result<u8> {
        self.ensure(1)?;
        let offset = self.pos_to_offset(self.pos);
        Ok(self.buf[offset])
    }

    pub fn read_1(&mut self) -> Result<u8> {
        self.ensure(1)?;

        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;

        Ok(buf[0])
    }

    pub fn read_4(&mut self) -> Result<u32> {
        self.ensure(4)?;

        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;

        Ok(mach::mach_read_from_4(&buf))
    }

    pub fn read_8(&mut self) -> Result<u64> {
        self.ensure(8)?;

        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;

        Ok(mach::mach_read_from_8(&buf))
    }
}

impl<'a> Read for RingReader<'a> {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize> {
        let offset0 = self.pos_to_offset(self.pos);
        let size1 = min(self.buf.len() - offset0, buf.len());
        buf[..size1].copy_from_slice(&self.buf[offset0..offset0 + size1]);

        self.pos += size1;
        if size1 == buf.len() {
            return Ok(size1);
        }

        buf = &mut buf[size1..];
        let size2 = min(offset0, buf.len());
        buf[0..size2].copy_from_slice(&self.buf[self.header..self.header + size2]);

        self.pos += size2;

        Ok(size1 + size2)
    }
}

impl<'a> Add<usize> for &RingReader<'a> {
    type Output = RingReader<'a>;

    fn add(self, bytes: usize) -> Self::Output {
        let mut new_reader = self.clone();
        new_reader.advance(bytes);
        new_reader
    }
}

/// returns the position in the header+ring_buffer for a given pos.
pub fn pos_to_offset(hdr: usize, body: usize, pos: usize) -> usize {
    if pos < hdr {
        return pos; // within the header
    }

    hdr + (pos - hdr) % body
}

#[cfg(test)]
mod test {
    use std::io::Read;

    use byteorder::ReadBytesExt;

    use super::RingReader;

    #[test]
    fn test_ring_reader() {
        let storage = [1u8, 2, 3, 4, 5];
        let buf = &storage;

        let r0 = RingReader::new(buf);
        let mut r1 = r0.clone();

        assert_eq!(r1.read_u8().unwrap(), 1);
        assert_eq!(r1.read_u8().unwrap(), 2, "{r1:#?}");
        assert_eq!(r1.read_u8().unwrap(), 3);
        assert_eq!(r1.read_u8().unwrap(), 4);
        assert_eq!(r1.read_u8().unwrap(), 5);

        let mut d2 = [0u8; 2];
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[1, 2]);
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[3, 4]);
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[5, 1]);

        let mut d4 = [0u8; 4];
        r1.read_exact(&mut d4).unwrap();
        assert_eq!(&d4, &[2, 3, 4, 5]);

        let mut d6 = [0u8; 6];
        #[allow(clippy::unused_io_amount)]
        r1.read(&mut d6).unwrap();
        assert_eq!(&d6, &[1, 2, 3, 4, 5, 0]);

        let r0 = RingReader::buf_at(buf, 1, 0);
        let mut r1 = r0.clone();

        assert_eq!(r1.read_u8().unwrap(), 1);
        assert_eq!(r1.read_u8().unwrap(), 2, "{r1:#?}");
        assert_eq!(r1.read_u8().unwrap(), 3);
        assert_eq!(r1.read_u8().unwrap(), 4);
        assert_eq!(r1.read_u8().unwrap(), 5);

        let r0 = RingReader::buf_at(buf, 1, 5);
        let mut r1 = r0.clone();

        assert_eq!(r1.read_u8().unwrap(), 2, "{r1:#?}");
        assert_eq!(r1.read_u8().unwrap(), 3);
        assert_eq!(r1.read_u8().unwrap(), 4);

        let mut d2 = [0u8; 2];
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[5, 2]);
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[3, 4]);
        r1.read_exact(&mut d2).unwrap();
        assert_eq!(&d2, &[5, 2]);

        let mut d4 = [0u8; 4];
        r1.read_exact(&mut d4).unwrap();
        assert_eq!(&d4, &[3, 4, 5, 2]);

        let mut d6 = [0u8; 6];
        #[allow(clippy::unused_io_amount)]
        r1.read(&mut d6).unwrap();
        assert_eq!(&d6, &[3, 4, 5, 2, 3, 0]);
    }

    #[test]
    fn test_from_end() {
        let storage = [1u8, 2, 3, 4, 5];
        let buf = &storage;
        let mut r0 = RingReader::buf_at(buf, 0, 5);

        assert_eq!(r0.read_u8().unwrap(), 1);

        let mut r0 = RingReader::buf_at(buf, 1, 5);
        assert_eq!(r0.pos_to_offset(5), 1);
        assert_eq!(r0.read_u8().unwrap(), 2);
    }
}
