use std::{
    cmp::min,
    io::{Error, ErrorKind, Read, Result, Seek, Write},
    ops::Add,
    ops::Index,
};

use crc32c::crc32c;
use mmap_rs::MmapMut;

use crate::mach;

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

    pub fn block(&self, mut buf: &mut [u8]) -> usize {
        if buf.len() > self.buf.len() {
            buf = &mut buf[..self.buf.len()];
        }

        let start = self.pos_to_offset(self.pos);
        let end = self.pos_to_offset(self.pos + buf.len());
        if start < end {
            buf.copy_from_slice(&self.buf[start..end]);
        } else {
            let size1 = self.buf.len() - start;
            buf[..size1].copy_from_slice(&self.buf[start..]);
            buf[size1..].copy_from_slice(&self.buf[self.header..end]);
        }

        buf.len()
    }

    pub fn crc32c(&self, size: usize) -> Result<u32> {
        let mut buf = vec![0u8; size];
        if self.block(&mut buf) != size {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }
        Ok(crc32c(&buf))
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

        if self.pos.checked_add(t).is_none() {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }

        Ok(())
    }

    pub fn advance(&mut self, bytes: usize) -> bool {
        // TODO: overflowing u64 pos.
        if let Some(new_pos) = self.pos.checked_add(bytes) {
            self.pos = new_pos;
            true
        } else {
            false
        }
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

    pub fn zero(&self, size: usize) -> bool {
        // memory copy is not efficient here, but ok.
        let mut buf = vec![0u8; size];
        self.block(&mut buf);
        buf.iter().all(|&b| b == 0)
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

impl<'a> Add<u32> for &RingReader<'a> {
    type Output = RingReader<'a>;

    fn add(self, bytes: u32) -> Self::Output {
        self + bytes as usize
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

impl<'a> Index<u32> for RingReader<'a> {
    type Output = u8;

    fn index(&self, index: u32) -> &Self::Output {
        self.index(index as usize)
    }
}

impl<'a> Index<usize> for RingReader<'a> {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        // TODO: use peek_1()
        // TODO: overflowing u64 pos.
        let Some(pos) = self.pos.checked_add(index) else {
            todo!("overflowing index access in RingReader");
        };
        let offset = self.pos_to_offset(pos);
        &self.buf[offset]
    }
}

/// returns the position in the header+ring_buffer for a given pos.
pub fn pos_to_offset(hdr: usize, body: usize, pos: usize) -> usize {
    if pos < hdr {
        return pos; // within the header
    }

    hdr + (pos - hdr) % body
}

#[derive(Debug)]
pub struct RingWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
    /// The size of the header in the beginning.
    header: usize,
}

impl<'a> RingWriter<'a> {
    pub fn new(buf: &'a mut [u8]) -> RingWriter<'a> {
        Self::buf_at(buf, 0, 0)
    }

    /// Creates a new `RingWriter` at the given position in the buffer.
    /// Buffer must be at least `hdr` bytes long and includes the header.
    pub fn buf_at(buf: &'a mut [u8], hdr: usize, pos: usize) -> RingWriter<'a> {
        RingWriter {
            buf,
            pos,
            header: hdr,
        }
    }

    /// returns the position in the header+ring_buffer for a given pos.
    pub fn pos_to_offset(&self, pos: usize) -> usize {
        pos_to_offset(self.header, self.buf.len() - self.header, pos)
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
}

impl<'a> Seek for RingWriter<'a> {
    fn seek(&mut self, pos: std::io::SeekFrom) -> Result<u64> {
        let new_pos = match pos {
            std::io::SeekFrom::Start(offset) => offset as usize,
            std::io::SeekFrom::End(offset) => {
                if offset > 0 && offset as usize > self.pos {
                    return Err(Error::from(ErrorKind::InvalidInput));
                }

                if offset < 0 {
                    self.pos + (-offset) as usize
                } else {
                    self.pos - offset as usize
                }
            }
            std::io::SeekFrom::Current(offset) => {
                if offset < 0 && self.pos < (-offset) as usize {
                    return Err(Error::from(ErrorKind::InvalidInput));
                }

                if offset < 0 {
                    self.pos - (-offset) as usize
                } else {
                    self.pos + offset as usize
                }
            }
        };

        self.pos = new_pos;

        Ok(self.pos as u64)
    }
}

impl<'a> Write for RingWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let offset0 = self.pos_to_offset(self.pos);
        let size1 = min(self.buf.len() - offset0, buf.len());
        self.buf[offset0..offset0 + size1].copy_from_slice(&buf[..size1]);

        self.pos += size1;
        if size1 == buf.len() {
            return Ok(size1);
        }

        let remaining = &buf[size1..];
        let size2 = min(offset0 - self.header, remaining.len());
        self.buf[self.header..self.header + size2].copy_from_slice(&remaining[..size2]);
        self.pos += size2;
        Ok(size1 + size2)
    }

    fn flush(&mut self) -> Result<()> {
        // No-op for ring buffer
        Ok(())
    }
}

pub struct MmapRingWriter {
    m: MmapMut,
    h: usize,
}

impl MmapRingWriter {
    pub fn new(m: MmapMut, h: usize) -> MmapRingWriter {
        MmapRingWriter { m, h }
    }

    pub fn writer(&mut self) -> RingWriter<'_> {
        RingWriter::buf_at(&mut self.m, self.h, 0)
    }
}

#[cfg(test)]
mod test {
    use std::io::{Read, Seek, Write};

    use byteorder::ReadBytesExt;

    use super::{RingReader, RingWriter};

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

    #[test]
    fn test_ring_writer() {
        let mut storage = [0u8; 10];
        let buf = &mut storage;

        let mut w0 = RingWriter::new(buf);
        assert_eq!(w0.write(&[1, 2, 3]).unwrap(), 3);
        assert_eq!(w0.pos(), 3);
        assert_eq!(&w0.buf[..3], &[1, 2, 3]);

        w0.seek(std::io::SeekFrom::Start(0)).unwrap();
        assert_eq!(w0.write(&[4, 5]).unwrap(), 2);
        assert_eq!(w0.pos(), 2);
        assert_eq!(&w0.buf[..5], &[4, 5, 3, 0, 0]);

        w0.seek(std::io::SeekFrom::Start(5)).unwrap();
        assert_eq!(w0.write(&[5, 6, 7, 8, 9]).unwrap(), 5);
        assert_eq!(w0.pos(), 10);
        assert_eq!(&w0.buf, &[4, 5, 3, 0, 0, 5, 6, 7, 8, 9]);

        assert_eq!(w0.write(&[4, 5]).unwrap(), 2);
        assert_eq!(w0.pos(), 12);
        assert_eq!(&w0.buf[..5], &[4, 5, 3, 0, 0]);

        w0.seek(std::io::SeekFrom::Start(6)).unwrap();
        assert_eq!(w0.write(&[6, 7, 8, 9, 10]).unwrap(), 5);
        assert_eq!(w0.pos(), 11);
        assert_eq!(&w0.buf, &[10, 5, 3, 0, 0, 5, 6, 7, 8, 9]);

        w0.seek(std::io::SeekFrom::Start(2)).unwrap();
        assert_eq!(w0.write(&[9]).unwrap(), 1);
        assert_eq!(w0.pos(), 3);
        assert_eq!(&w0.buf, &[10, 5, 9, 0, 0, 5, 6, 7, 8, 9]);

        w0.seek(std::io::SeekFrom::Start(10)).unwrap();
        w0.seek(std::io::SeekFrom::Current(1)).unwrap();
        assert_eq!(w0.pos(), 11);
        w0.seek(std::io::SeekFrom::Current(-2)).unwrap();
        assert_eq!(w0.pos(), 9);
        w0.seek(std::io::SeekFrom::End(1)).unwrap();
        assert_eq!(w0.pos(), 8);
        w0.seek(std::io::SeekFrom::End(-1)).unwrap();
        assert_eq!(w0.pos(), 9);
    }
}
