use std::{
    fmt::Display,
    io::{Error, ErrorKind, Result},
    path::Path,
};

use anyhow::Context;
use crc32c::crc32c;
use mmap_rs::{Mmap, MmapFlags, MmapOptions};

use crate::fil0fil;
use crate::fsp0fsp;
use crate::fsp0types;
use crate::mach;
use crate::page0page;

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
                        "Inconsistent tablespace ID or flags in file, expected (space_id={}, flags={:#x}) but found (space_id={}, flags={:#x})",
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
                    "InnoDB: Data file uses page size {}, but the innodb_page_size start-up parameter is {}",
                    logical_size, self.page
                ),
            ));
        }

        let page0_ptr = 0;
        if page0page::page_get_page_no(self.buf, page0_ptr, self.page) != 0 {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!(
                    "InnoDB: Header pages contains inconsistent data (page number is not 0), Space ID: {}, Flags: {}",
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

        // TODO: page struct (buf: &[u8], page_id, page_ptr)
        let page0_buf = self.page(0)?;
        self.buf_page_is_corrupted(false, page0_buf)?;

        Ok(())
    }

    /// Check whether a page is corrupted.
    /// Reference: buf0buf.cc:buf_page_is_corrupted().
    pub fn buf_page_is_corrupted(&self, _check_lsn: bool, page: &[u8]) -> Result<()> {
        if fil0fil::full_crc32(self.flags) {
            let (page_size, _compressed, corrupted) = Self::buf_page_full_crc32_size(page);
            if corrupted {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "InnoDB: Page is corrupted (full CRC32 size)",
                ));
            }

            let end = &page[page_size - fil0fil::FIL_PAGE_FCRC32_CHECKSUM as usize..];
            let crc32 = mach::mach_read_from_4(end);

            // A full size page filled with NUL bytes is considered not corrupted and it does not have
            // checksum.
            if crc32 == 0 && page_size == self.page && page[..page_size].iter().all(|&b| b == 0) {
                return Ok(());
            }

            if crc32c(&page[..page_size - fil0fil::FIL_PAGE_FCRC32_CHECKSUM as usize]) != crc32 {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "InnoDB: Page is corrupted (full CRC32 checksum mismatch)",
                ));
            }

            /*
            static_assert(FIL_PAGE_FCRC32_KEY_VERSION == 0, "alignment");
            static_assert(FIL_PAGE_LSN % 4 == 0, "alignment");
            static_assert(FIL_PAGE_FCRC32_END_LSN % 4 == 0, "alignment");
            if (!compressed
                && !mach_read_from_4(FIL_PAGE_FCRC32_KEY_VERSION
                   + read_buf)
                && memcmp_aligned<4>(read_buf + (FIL_PAGE_LSN + 4),
                   end - (FIL_PAGE_FCRC32_END_LSN
                    - FIL_PAGE_FCRC32_CHECKSUM),
                   4)) {
              return CORRUPTED_OTHER;
            }

            return
            #ifndef UNIV_INNOCHECKSUM
                  buf_page_check_lsn(check_lsn, read_buf)
                  ? CORRUPTED_FUTURE_LSN :
            #endif
                  NOT_CORRUPTED;
            */
        }

        /*
          const ulint zip_size = fil_space_t::zip_size(fsp_flags);
          const uint16_t page_type = fil_page_get_type(read_buf);

          /* We can trust page type if page compression is set on tablespace
          flags because page compression flag means file must have been
          created with 10.1 (later than 5.5 code base). In 10.1 page
          compressed tables do not contain post compression checksum and
          FIL_PAGE_END_LSN_OLD_CHKSUM field stored. Note that space can
          be null if we are in fil_check_first_page() and first page
          is not compressed or encrypted. Page checksum is verified
          after decompression (i.e. normally pages are already
          decompressed at this stage). */
          if ((page_type == FIL_PAGE_PAGE_COMPRESSED ||
               page_type == FIL_PAGE_PAGE_COMPRESSED_ENCRYPTED)
        #ifndef UNIV_INNOCHECKSUM
              && FSP_FLAGS_HAS_PAGE_COMPRESSION(fsp_flags)
        #endif
          ) {
          check_lsn:
            return
        #ifndef UNIV_INNOCHECKSUM
              buf_page_check_lsn(check_lsn, read_buf)
              ? CORRUPTED_FUTURE_LSN :
        #endif
              NOT_CORRUPTED;
          }

          static_assert(FIL_PAGE_LSN % 4 == 0, "alignment");
          static_assert(FIL_PAGE_END_LSN_OLD_CHKSUM % 4 == 0, "alignment");

          if (!zip_size
              && memcmp_aligned<4>(read_buf + FIL_PAGE_LSN + 4,
                 read_buf + srv_page_size
                 - FIL_PAGE_END_LSN_OLD_CHKSUM + 4, 4)) {
            /* Stored log sequence numbers at the start and the end
            of page do not match */

            return CORRUPTED_OTHER;
          }

          /* Check whether the checksum fields have correct values */

          if (zip_size) {
            if (!page_zip_verify_checksum(read_buf, zip_size)) {
              return CORRUPTED_OTHER;
            }
            goto check_lsn;
          }

          const uint32_t checksum_field1 = mach_read_from_4(
            read_buf + FIL_PAGE_SPACE_OR_CHKSUM);

          const uint32_t checksum_field2 = mach_read_from_4(
            read_buf + srv_page_size - FIL_PAGE_END_LSN_OLD_CHKSUM);

          static_assert(FIL_PAGE_LSN % 8 == 0, "alignment");

          /* A page filled with NUL bytes is considered not corrupted.
          Before MariaDB Server 10.1.25 (MDEV-12113) or 10.2.2 (or MySQL 5.7),
          the FIL_PAGE_FILE_FLUSH_LSN field may have been written nonzero
          for the first page of each file of the system tablespace.
          We want to ignore it for the system tablespace, but because
          we do not know the expected tablespace here, we ignore the
          field for all data files, except for
          innodb_checksum_algorithm=full_crc32 which we handled above. */
          if (!checksum_field1 && !checksum_field2) {
            /* Checksum fields can have valid value as zero.
            If the page is not empty then do the checksum
            calculation for the page. */
            bool all_zeroes = true;
            for (size_t i = 0; i < srv_page_size; i++) {
        #ifndef UNIV_INNOCHECKSUM
              if (i == FIL_PAGE_FILE_FLUSH_LSN_OR_KEY_VERSION) {
                i += 8;
              }
        #endif
              if (read_buf[i]) {
                all_zeroes = false;
                break;
              }
            }

            if (all_zeroes) {
              return NOT_CORRUPTED;
            }
          }

        #ifndef UNIV_INNOCHECKSUM
          switch (srv_checksum_algorithm) {
          case SRV_CHECKSUM_ALGORITHM_STRICT_FULL_CRC32:
          case SRV_CHECKSUM_ALGORITHM_STRICT_CRC32:
        #endif /* !UNIV_INNOCHECKSUM */
            if (!buf_page_is_checksum_valid_crc32(read_buf,
                          checksum_field1,
                          checksum_field2)) {
              return CORRUPTED_OTHER;
            }
            goto check_lsn;
        #ifndef UNIV_INNOCHECKSUM
          default:
            if (checksum_field1 == BUF_NO_CHECKSUM_MAGIC
                && checksum_field2 == BUF_NO_CHECKSUM_MAGIC) {
              goto check_lsn;
            }

            const uint32_t crc32 = buf_calc_page_crc32(read_buf);

            /* Very old versions of InnoDB only stored 8 byte lsn to the
            start and the end of the page. */

            /* Since innodb_checksum_algorithm is not strict_* allow
            any of the algos to match for the old field */

            if (checksum_field2
                != mach_read_from_4(read_buf + FIL_PAGE_LSN)
                && checksum_field2 != BUF_NO_CHECKSUM_MAGIC) {

              DBUG_EXECUTE_IF(
                "page_intermittent_checksum_mismatch", {
                static int page_counter;
                if (mach_read_from_4(FIL_PAGE_OFFSET
                         + read_buf)
                    && page_counter++ == 6)
                  return CORRUPTED_OTHER;
              });

              if ((checksum_field1 != crc32
                   || checksum_field2 != crc32)
                  && checksum_field2
                  != buf_calc_page_old_checksum(read_buf)) {
                return CORRUPTED_OTHER;
              }
            }

            switch (checksum_field1) {
            case 0:
            case BUF_NO_CHECKSUM_MAGIC:
              break;
            default:
              if ((checksum_field1 != crc32
                   || checksum_field2 != crc32)
                  && checksum_field1
                  != buf_calc_page_new_checksum(read_buf)) {
                return CORRUPTED_OTHER;
              }
            }
          }
        #endif /* !UNIV_INNOCHECKSUM */
          goto check_lsn;
        */

        Ok(())
    }

    /// Get the compressed or uncompressed size of a full_crc32 page.
    ///
    /// # Arguments
    /// * `buf` - page_compressed or uncompressed page
    /// * `comp` - mutable reference, set to true if the page could be compressed
    /// * `cr` - mutable reference, set to true if the page could be corrupted
    ///
    /// # Returns
    /// The payload size in the file page, whether the page could be compressed, and whether the
    /// page could be corrupted.
    fn buf_page_full_crc32_size(page: &[u8]) -> (usize, bool, bool) {
        let mut page_type = fil0fil::fil_page_get_type(page) as u32;
        let mut page_size = page.len();
        let mut compressed = false;
        let mut corrupted = false;

        if (page_type & (1u32 << fil0fil::FIL_PAGE_COMPRESS_FCRC32_MARKER)) == 0 {
            return (page_size, compressed, corrupted);
        }

        page_type &= !(1u32 << fil0fil::FIL_PAGE_COMPRESS_FCRC32_MARKER);
        page_type <<= 8;

        if (page_type as usize) < page_size {
            page_size = page_type as usize;
            compressed = true;
        } else {
            corrupted = true;
        }

        (page_size, compressed, corrupted)
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

    pub fn page(&self, page_id: u32) -> Result<&[u8]> {
        let pos = (page_id as usize)
            .checked_mul(self.page)
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "page_id overflow"))?;
        self.block(pos, self.page)
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
