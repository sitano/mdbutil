use std::cmp::min;
use std::io::{Error, ErrorKind, Read, Result};

use crate::mach;

#[derive(Debug, Clone)]
pub struct RingReader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> RingReader<'a> {
    // TODO: base offset (header)
    pub fn new(buf: &'a [u8]) -> RingReader<'a> {
        Self::buf_at(buf, 0)
    }

    pub fn buf_at(buf: &'a [u8], pos: usize) -> RingReader<'a> {
        RingReader {
            buf,
            pos: pos % buf.len(),
        }
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
        self.pos = (self.pos + bytes) % self.buf.len();
    }

    pub fn peek_1(&self) -> Result<u8> {
        self.ensure(1)?;
        Ok(self.buf[self.pos])
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
        let pos0 = self.pos;
        let size1 = min(self.buf.len() - self.pos, buf.len());
        buf[..size1].copy_from_slice(&self.buf[self.pos..self.pos + size1]);

        self.pos += size1;
        if self.pos >= self.buf.len() {
            self.pos = 0;
        } else {
            return Ok(size1);
        }

        buf = &mut buf[size1..];
        let size2 = min(pos0, buf.len());
        buf[0..size2].copy_from_slice(&self.buf[..size2]);

        self.pos += size2;

        Ok(size1 + size2)
    }
}

#[cfg(test)]
mod test {
    use byteorder::ReadBytesExt;
    use std::io::Read;

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
    }

    #[test]
    fn test_from_end() {
        let storage = [1u8, 2, 3, 4, 5];
        let buf = &storage;
        let mut r0 = RingReader::buf_at(buf, 5);

        assert_eq!(r0.read_u8().unwrap(), 1);
    }
}
