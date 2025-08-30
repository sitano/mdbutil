use std::io::{Error, ErrorKind, Result};

use crc32c::crc32c;

use crate::Lsn;
use crate::fil0fil;
use crate::mach;
use crate::page_buf::PageBuf;

/// Check whether a page is corrupted.
/// Reference: buf0buf.cc:buf_page_is_corrupted().
pub fn buf_page_is_corrupted(page: &PageBuf, _check_lsn: Option<Lsn>) -> Result<()> {
    if fil0fil::full_crc32(page.flags()) {
        let (page_size, _compressed, corrupted) = buf_page_full_crc32_size(page);
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
        if crc32 == 0 && page_size == page.page_size() && page[..page_size].iter().all(|&b| b == 0)
        {
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
fn buf_page_full_crc32_size(page: &PageBuf) -> (usize, bool, bool) {
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
