use std::{
    fmt::Display,
    io::{Error, ErrorKind, Result},
    path::Path,
};

use anyhow::Context;
use mmap_rs::{Mmap, MmapFlags, MmapOptions};

use crate::fil0fil;
use crate::fsp0fsp;
use crate::mach;

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
                    "Inconsistent tablespace ID in file, expected {fil_page_space_id} but found {fsp_header_space_id}",
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

    pub fn ensure(&self, pos: usize, len: usize) -> Result<()> {
        match pos.checked_add(len) {
            Some(end) if end <= self.buf.len() => Ok(()),
            Some(_) => Err(Error::from(ErrorKind::UnexpectedEof)),
            None => Err(Error::from(ErrorKind::UnexpectedEof)),
        }
    }

    pub fn block(&self, pos: usize, len: usize) -> Result<&[u8]> {
        self.ensure(pos, len)?;

        Ok(&self.buf[pos..pos + len])
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

    pub fn reader(&self) -> anyhow::Result<TablespaceReader<'_>> {
        let mut reader = TablespaceReader::new(self.m.as_slice(), self.page);

        reader
            .parse_first_page()
            .context("parse first page of tablespace")?;

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
