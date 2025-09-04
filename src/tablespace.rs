#![allow(clippy::len_without_is_empty)]

use std::{
    fmt::Display,
    io::{Error, ErrorKind, Result},
    ops::Range,
    path::Path,
};

use anyhow::Context;
use mmap_rs::{Mmap, MmapFlags, MmapMut, MmapOptions};

use crate::{fil0fil, fsp0fsp, fsp0types, mach, page_buf::PageBuf, page0page};

#[derive(Debug, Clone)]
pub struct TablespaceReader<'a> {
    buf: &'a [u8],
    /// page size, by default 16K.
    page: usize,
    // pages: u64,
    /// The order of the datafile in the tablespace.
    /// See Datafile::m_order in fsp0file.h.
    order: usize,
    /// tablespace id
    space_id: u32,
    /// tablespace flags
    flags: u32,
}

impl<'a> TablespaceReader<'a> {
    pub fn new(buf: &'a [u8], page: usize) -> TablespaceReader<'a> {
        TablespaceReader {
            buf,
            page,
            // See SysTablespace::check_size().
            // pages: (buf.len() / page) as u64,
            order: 0,
            space_id: 0,
            flags: 0,
        }
    }

    // Reads a few significant fields from the first page of the first
    // datafile. Reference: fsp0file.cc:Datafile::read_first_page().
    pub fn parse_first_page(&mut self) -> Result<()> {
        if self.order == 0 {
            (self.space_id, self.flags) = self.read_first_page_flags()?;
        }

        if fil0fil::physical_size(self.flags, self.page) > self.page {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("File should be longer than {} bytes", self.page),
            ));
        }

        Ok(())
    }

    // Reference: fsp0file.cc:Datafile::read_first_page_flags().
    pub fn read_first_page_flags(&self) -> Result<(u32, u32)> {
        assert!(self.order == 0, "order must be 0");

        let fil_page_space_id = self.read_4(fil0fil::FIL_PAGE_SPACE_ID as usize)?;
        let fsp_header_space_id =
            self.read_4((fsp0fsp::FSP_HEADER_OFFSET + fsp0fsp::FSP_SPACE_ID) as usize)?;

        if fil_page_space_id != fsp_header_space_id {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "Inconsistent tablespace ID in file, expected {fil_page_space_id} but found \
                     {fsp_header_space_id}",
                ),
            ));
        }

        // Check space ID and flags.
        let space_id = self.read_4(fil0fil::FIL_PAGE_SPACE_ID as usize)?;
        let flags =
            self.read_4((fsp0fsp::FSP_HEADER_OFFSET + fsp0fsp::FSP_SPACE_FLAGS) as usize)?;
        // is_ibd is true if this is an .ibd file (not the system tablespace).
        let is_ibd = space_id != 0;

        if !fil0fil::is_valid_flags(flags, is_ibd, self.page) {
            // original code tries to convert flags from old version (fsp_flags_convert_from_101).
            // we don't need that.
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Invalid tablespace flags: {flags:#x}"),
            ));
        }

        Ok((space_id, flags))
    }

    /// Check the consistency of the first page of a datafile when the tablespace is opened. This
    /// occurs before the fil_space_t is created so the Space ID found here must not already be
    /// open. m_is_valid is set true on success, else false. Reference:
    /// fsp0file.cc:Datafile::validate_first_page().
    ///
    /// # Arguments
    /// * `first_page` - the contents of the first page
    pub fn validate_first_page(&self) -> Result<()> {
        // Instead of guessing if we had a call to read_first_page()
        // always check consistency of the read_first_page_flags().
        if self.order == 0 {
            let (space_id, flags) = self.read_first_page_flags()?;

            if space_id != self.space_id || flags != self.flags {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "Inconsistent tablespace ID or flags in file, expected (space_id={}, \
                         flags={:#x}) but found (space_id={}, flags={:#x})",
                        self.space_id, self.flags, space_id, flags
                    ),
                ));
            }
        }

        if fil0fil::physical_size(self.flags, self.page) > self.page {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnodDB: File should be longer than {} bytes, Space ID: {}, Flags: {}",
                    self.page, self.space_id, self.flags
                ),
            ));
        }

        // Check if the whole page is blank.
        if self.space_id == 0 && self.flags == 0 {
            let mut nonzero_bytes = self.page;

            while nonzero_bytes > 0 && self.buf[nonzero_bytes - 1] == 0 {
                nonzero_bytes -= 1;
            }

            if nonzero_bytes == 0 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "InnoDB: Header page consists of zero bytes in Space ID: {}, Flags: {}",
                        self.space_id, self.flags
                    ),
                ));
            }
        }

        // is_ibd is true if this is an .ibd file (not the system tablespace).
        let is_ibd = self.space_id != 0;

        if !fil0fil::is_valid_flags(self.flags, is_ibd, self.page) {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnoDB: Tablespace flags are invalid in Space ID: {}, Flags: {}",
                    self.space_id, self.flags
                ),
            ));
        }

        let logical_size = fil0fil::logical_size(self.flags);

        if self.page != logical_size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnoDB: Data file uses page size {}, but the innodb_page_size start-up \
                     parameter is {}",
                    logical_size, self.page
                ),
            ));
        }

        let page0_ptr = 0;
        if page0page::page_get_page_no(self.buf, page0_ptr, self.page) != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnoDB: Header pages contains inconsistent data (page number is not 0), \
                     Space ID: {}, Flags: {}",
                    self.space_id, self.flags
                ),
            ));
        }

        if self.space_id >= fsp0types::SRV_SPACE_ID_UPPER_BOUND {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnoDB: A bad Space ID was found, Space ID: {}, Flags: {}",
                    self.space_id, self.flags
                ),
            ));
        }

        let page = self.page(0)?;

        page.corrupted(None)?;

        Ok(())
    }

    pub fn ensure(&self, pos: usize, len: usize) -> Result<()> {
        match pos.checked_add(len) {
            Some(end) if end <= self.buf.len() => Ok(()),
            Some(_) => Err(Error::from(ErrorKind::UnexpectedEof)),
            None => Err(Error::from(ErrorKind::UnexpectedEof)),
        }
    }

    pub fn block(&self, pos: usize, len: usize) -> Result<&'a [u8]> {
        self.ensure(pos, len)?;

        Ok(&self.buf[pos..pos + len])
    }

    pub fn page(&self, page_no: u32) -> Result<PageBuf<'a>> {
        let pos = (page_no as usize)
            .checked_mul(self.page)
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "page_id overflow"))?;

        Ok(PageBuf::new(self.flags, self.block(pos, self.page)?))
    }

    pub fn read_4(&self, pos: usize) -> Result<u32> {
        Ok(mach::mach_read_from_4(self.block(pos, 4)?))
    }

    pub fn order(&self) -> usize {
        self.order
    }

    pub fn space_id(&self) -> u32 {
        self.space_id
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }
}

pub struct MmapTablespaceReader {
    m: Mmap,
    page: usize,
}

impl MmapTablespaceReader {
    pub fn new(m: Mmap, page: usize) -> MmapTablespaceReader {
        MmapTablespaceReader { m, page }
    }

    pub fn open(file_path: &Path, page_size: usize) -> anyhow::Result<MmapTablespaceReader> {
        let file = std::fs::File::open(file_path)
            .with_context(|| format!("open tablespace at {}", file_path.display()))?;
        let meta = file
            .metadata()
            .context("get metadata for tablespace a file")?;
        let size = meta.len();

        if page_size == 0 {
            return Err(anyhow::anyhow!("tablespace file is empty"));
        }

        if size % page_size as u64 != 0 {
            return Err(anyhow::anyhow!(
                "tablespace file size {size} is not a multiple of page size {page_size}",
            ));
        }

        let mmap = unsafe {
            MmapOptions::new(size as usize)
                .context("mmap option")?
                .with_file(&file, 0u64)
                .with_flags(MmapFlags::SHARED)
                .map()
                .context("mmap tablespace file")?
        };

        Ok(MmapTablespaceReader::new(mmap, page_size))
    }

    pub fn mmap(&self) -> &Mmap {
        &self.m
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn reader(&self) -> anyhow::Result<TablespaceReader<'_>> {
        let mut reader = TablespaceReader::new(self.m.as_slice(), self.page);

        reader
            .parse_first_page()
            .context("parse first page of tablespace")?;

        reader
            .validate_first_page()
            .context("validate first page of tablespace")?;

        Ok(reader)
    }
}

impl Display for TablespaceReader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tablespace(space_id={}, flags={:#x}, page_size={}, order={})",
            self.space_id, self.flags, self.page, self.order
        )
    }
}

pub struct MmapTablespaceWriter {
    m: MmapMut,
    page: usize,
}

impl MmapTablespaceWriter {
    pub fn new(m: MmapMut, page: usize) -> MmapTablespaceWriter {
        MmapTablespaceWriter { m, page }
    }

    pub fn open(file_path: &Path, page_size: usize) -> anyhow::Result<MmapTablespaceWriter> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(file_path)
            .with_context(|| format!("open log file at {}", file_path.display()))?;

        let meta = file_path
            .metadata()
            .context("get metadata for tablespace a file")?;
        let size = meta.len();

        if page_size == 0 {
            return Err(anyhow::anyhow!("tablespace file is empty"));
        }

        if size % page_size as u64 != 0 {
            return Err(anyhow::anyhow!(
                "tablespace file size {size} is not a multiple of page size {page_size}",
            ));
        }

        let mmap = unsafe {
            MmapOptions::new(size as usize)
                .context("mmap option")?
                .with_file(&file, 0u64)
                .with_flags(MmapFlags::SHARED)
                .map_mut()
                .context("mmap tablespace file")?
        };

        Ok(MmapTablespaceWriter::new(mmap, page_size))
    }

    pub fn mmap_mut(&self) -> &MmapMut {
        &self.m
    }

    pub fn len(&self) -> usize {
        self.m.len()
    }

    pub fn flush(&self, range: Range<usize>) -> anyhow::Result<()> {
        self.m.flush(range)?;
        Ok(())
    }

    pub fn flush_all(&self) -> anyhow::Result<()> {
        self.m.flush(0..self.len())?;
        Ok(())
    }

    pub fn reader(&self) -> anyhow::Result<TablespaceReader<'_>> {
        let mut reader = TablespaceReader::new(self.m.as_slice(), self.page);

        reader
            .parse_first_page()
            .context("parse first page of tablespace")?;

        reader
            .validate_first_page()
            .context("validate first page of tablespace")?;

        Ok(reader)
    }

    pub fn writer(&mut self) -> anyhow::Result<TablespaceWriter<'_>> {
        let reader = self.reader()?;

        let space_id = reader.space_id();
        let flags = reader.flags();

        let mut writer = TablespaceWriter::new(self.m.as_mut_slice(), self.page, space_id, flags);

        writer.space_id = space_id;
        writer.flags = flags;

        Ok(writer)
    }
}

// TODO: implement Writer+Seek
#[derive(Debug)]
pub struct TablespaceWriter<'a> {
    buf: &'a mut [u8],
    /// page size, by default 16K.
    page_size: usize,
    /// tablespace id
    space_id: u32,
    /// tablespace flags
    flags: u32,
}

impl<'a> TablespaceWriter<'a> {
    pub fn new(
        buf: &'a mut [u8],
        page_size: usize,
        space_id: u32,
        flags: u32,
    ) -> TablespaceWriter<'a> {
        TablespaceWriter {
            buf,
            page_size,
            space_id,
            flags,
        }
    }

    pub fn page_buf(&'a mut self, page_no: u32) -> Result<&'a mut [u8]> {
        let pos = (page_no as usize)
            .checked_mul(self.page_size)
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "page_id overflow"))?;

        if pos + self.page_size > self.buf.len() {
            return Err(Error::from(ErrorKind::UnexpectedEof));
        }

        Ok(&mut self.buf[pos..pos + self.page_size])
    }

    pub fn mmap_mut(&'a mut self) -> &'a mut [u8] {
        self.buf
    }

    pub fn page_size(&self) -> usize {
        self.page_size
    }

    pub fn space_id(&self) -> u32 {
        self.space_id
    }

    pub fn flags(&self) -> u32 {
        self.flags
    }
}

impl Display for TablespaceWriter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tablespace(space_id={}, page_size={}, pages={}, flags={:#x})",
            self.space_id,
            self.page_size,
            self.buf.len() / self.page_size,
            self.flags,
        )
    }
}
