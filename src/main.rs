use std::{
    io::{Seek, Write},
    path::PathBuf,
};

use clap::Parser;
use mdbutil::{
    Lsn,
    config::Config,
    fil0fil::{
        FIL_PAGE_TYPE_FSP_HDR, FIL_PAGE_TYPE_SYS, FIL_PAGE_TYPE_TRX_SYS, FIL_PAGE_UNDO_LOG,
        tablespace_flags_to_string,
    },
    fsp0fsp::fsp_header_t,
    fsp0types::FSP_TRX_SYS_PAGE_NO,
    log,
    log::{CHECKPOINT_1, CHECKPOINT_2, Redo, RedoHeader},
    mtr::Mtr,
    mtr0types::MtrOperation,
    page_buf::PageBuf,
    ring,
    tablespace::{MmapTablespaceReader, TablespaceReader},
    trx0rseg::trx_rseg_t,
    trx0sys::{trx_sys_rseg_t, trx_sys_t},
    trx0undo::trx_undo_page_t,
};

#[derive(Parser)]
enum Cli {
    ReadRedo(ReadRedoCommand),
    WriteRedo(WriteRedoCommand),
    ReadTablespace(ReadTablespaceCommand),
    ReadPage(ReadPageCommand),
}

#[derive(clap::Args)]
struct ReadRedoCommand {
    #[clap(flatten)]
    config: Config,
}

#[derive(clap::Args)]
struct WriteRedoCommand {
    #[clap(flatten)]
    config: Config,

    #[clap(long = "size", help = "Size of the redo log file in bytes")]
    size: u64,

    #[clap(
        long = "lsn",
        help = "Redo log sequence number (LSN). Usually is MariaDB sequence number - 16."
    )]
    lsn: Lsn,
}

#[derive(clap::Args)]
struct ReadTablespaceCommand {
    #[clap(
        long = "file-path",
        help = "Path to the tablespace file (ibdata1, undoXXX, *.ibd)"
    )]
    pub file_path: PathBuf,

    #[clap(
        long = "page-size",
        help = "Page size in bytes (default: 16384)",
        default_value = "16384"
    )]
    pub page_size: usize,

    #[clap(
        long = "undo-log-dir",
        help = "Path to the undo logs directory (Undo Log)"
    )]
    pub undo_log_dir: Option<PathBuf>,
}

#[derive(clap::Args)]
struct ReadPageCommand {
    #[clap(
        long = "file-path",
        help = "Path to the tablespace file (ibdata1, undoXXX, *.ibd)"
    )]
    pub file_path: PathBuf,

    #[clap(
        long = "page-size",
        help = "Page size in bytes (default: 16384)",
        default_value = "16384"
    )]
    pub page_size: usize,

    #[clap(
        long = "page",
        help = "Page number to read (0-based)",
        default_value = "0"
    )]
    pub page: u32,

    #[clap(
        long = "hex",
        help = "Dump page in hex format",
        default_value_t = false
    )]
    pub hex: bool,

    #[clap(long = "raw", help = "Dump raw page data", default_value_t = false)]
    pub raw: bool,
}

fn main() {
    let cli = Cli::parse();
    match cli {
        Cli::ReadRedo(cmd) => cmd.run(),
        Cli::WriteRedo(cmd) => cmd.run().expect("Failed to write redo log"),
        Cli::ReadTablespace(cmd) => cmd.run().expect("Failed to read tablespace"),
        Cli::ReadPage(cmd) => cmd.run().expect("Failed to read page"),
    };
}

impl ReadRedoCommand {
    fn run(self) {
        let log_file_path = self
            .config
            .get_log_file_path()
            .expect("Redo log file path not specified");
        let log = log::Redo::open(&log_file_path).expect("Failed to open redo log");

        println!("Header block: {}", log.header().first_lsn);
        println!("Size: {}, Capacity: {}", log.size(), log.capacity());

        println!("{:#?}", log.header());
        println!("{:#?}", log.checkpoint());

        let mut file_checkpoint_chain = None;
        let mut file_checkpoint_lsn = None;
        let mut reader = log.reader();
        let mut chains = 0usize;
        loop {
            let chain = match reader.parse_next() {
                Ok(chain) => chain,
                Err(err) => {
                    // test for EOM.
                    if let Some(err) = err.downcast_ref::<std::io::Error>()
                        && err.kind() == std::io::ErrorKind::NotFound
                    {
                        break;
                    }

                    eprintln!("ERROR: {err}: {:?}", err.source());
                    break;
                }
            };

            chains += 1;
            println!(
                "{}: MTR Chain count={}, len={}, lsn={}",
                chains,
                chain.mtr.len(),
                chain.len,
                chain.lsn
            );

            let mut i = 0;
            for mtr in &chain.mtr {
                if mtr.op == MtrOperation::FileCheckpoint
                    && Some(mtr.lsn) == log.checkpoint().checkpoint_lsn
                {
                    file_checkpoint_chain = Some(chain.clone());
                    file_checkpoint_lsn = mtr.file_checkpoint_lsn;
                }

                i += 1;
                println!(
                    "  {i}: [{start}..{end}) {mtr}",
                    start = reader.reader().pos_to_offset(mtr.lsn as usize),
                    end = reader
                        .reader()
                        .pos_to_offset(mtr.lsn as usize + mtr.len as usize),
                );
            }
        }

        println!("Checkpoint LSN/1: {:?}", log.checkpoint().checkpoints[0]);
        println!("Checkpoint LSN/2: {:?}", log.checkpoint().checkpoints[1]);

        if let Some(file_checkpoint_lsn) = file_checkpoint_lsn {
            println!("File checkpoint chain: {file_checkpoint_chain:?}");
            println!("File checkpoint LSN: {file_checkpoint_lsn}");
        } else {
            eprintln!("WARNING: No file checkpoint found in redo log.");
        }

        if log.header().version != log::FORMAT_10_8 {
            eprintln!("WARNING: the redo log is not in 10.8 format.");
        }

        if log.checkpoint().checkpoint_lsn != Some(log.checkpoint().end_lsn) {
            eprintln!("WARNING: checkpoint LSN is not at the end of the log.");
        }
    }
}

impl WriteRedoCommand {
    fn run(&self) -> anyhow::Result<()> {
        let path = self.config.get_log_file_path()?;

        let first_lsn = log::FIRST_LSN;
        let size = self.size;
        let capacity = size - first_lsn;

        let mut log = Redo::writer(path.as_path(), first_lsn as usize, size)
            .map_err(std::io::Error::other)?;
        let mut writer = log.writer();

        let header = RedoHeader::build_unencrypted_header_10_8(first_lsn, "test_creator")?;
        writer.seek(std::io::SeekFrom::Start(0))?;
        writer.write_all(&header)?;

        let checkpoint = RedoHeader::build_unencrypted_header_10_8_checkpoint(self.lsn, self.lsn)?;
        writer.seek(std::io::SeekFrom::Start(CHECKPOINT_1 as u64))?;
        writer.write_all(&checkpoint)?;

        writer.seek(std::io::SeekFrom::Start(CHECKPOINT_2 as u64))?;
        writer.write_all(&checkpoint)?;

        let mut file_checkpoint = vec![];
        Mtr::build_file_checkpoint(&mut file_checkpoint, first_lsn, capacity, self.lsn).unwrap();
        file_checkpoint.push(0x0); // end marker

        writer.seek(std::io::SeekFrom::Start(self.lsn))?;
        writer.write_all(&file_checkpoint)?;

        log.mmap().flush(0..size as usize)?;

        drop(log);

        println!(
            "Writing file checkpoint: {file_checkpoint:x?} at pos: {target_offset} \
             ({target_offset:#x})",
            target_offset =
                ring::pos_to_offset(first_lsn as usize, capacity as usize, self.lsn as usize)
        );

        let target_log = Redo::open(&path).expect("Failed to open target redo log");

        println!("Target header block: {}", target_log.header().first_lsn);
        println!(
            "Size: {}, Capacity: {:#x}",
            target_log.size(),
            target_log.capacity()
        );

        println!("{:#?}", target_log.header());
        println!("{:#?}", target_log.checkpoint());

        let mut file_checkpoint_lsn = None;
        let mut reader = target_log.reader();
        loop {
            let chain = match reader.parse_next() {
                Ok(chain) => chain,
                Err(err) => {
                    // test for EOM.
                    if let Some(err) = err.downcast_ref::<std::io::Error>()
                        && err.kind() == std::io::ErrorKind::NotFound
                    {
                        break;
                    }

                    eprintln!("\nERROR: {err:?}");
                    break;
                }
            };

            for mtr in chain.mtr {
                if mtr.op == MtrOperation::FileCheckpoint
                    && Some(mtr.lsn) == target_log.checkpoint().checkpoint_lsn
                {
                    file_checkpoint_lsn = mtr.file_checkpoint_lsn;
                }

                println!(
                    "  [{start}..{end}) {mtr}",
                    start = reader.reader().pos_to_offset(mtr.lsn as usize),
                    end = reader
                        .reader()
                        .pos_to_offset(mtr.lsn as usize + mtr.len as usize),
                );
            }
        }

        println!(
            "Target checkpoint LSN/1: {:?}",
            target_log.checkpoint().checkpoints[0]
        );
        println!(
            "Target checkpoint LSN/2: {:?}",
            target_log.checkpoint().checkpoints[1]
        );

        let file_checkpoint_lsn =
            file_checkpoint_lsn.expect("No file checkpoint found in redo target_log") as Lsn;
        println!("Target file checkpoint LSN: {file_checkpoint_lsn}");

        Ok(())
    }
}

impl ReadTablespaceCommand {
    fn run(&self) -> anyhow::Result<()> {
        let file_path = &self.file_path;
        let page_size = self.page_size;

        let mmap_reader: MmapTablespaceReader =
            mdbutil::tablespace::MmapTablespaceReader::open(file_path, page_size)?;
        let num_pages = mmap_reader.mmap().len() / page_size;

        let reader: TablespaceReader<'_> = mmap_reader.reader()?;

        println!(
            "Opened tablespace file: {} with size: {} bytes, page size: {} bytes, num pages: {}, \
             flags: {}",
            file_path.display(),
            mmap_reader.mmap().len(),
            page_size,
            num_pages,
            tablespace_flags_to_string(reader.flags()),
        );

        println!("{}", reader);

        let page: PageBuf<'_> = reader.page(0)?;
        println!("{}", page);

        if page.page_type == FIL_PAGE_TYPE_FSP_HDR {
            let fsp_header = fsp_header_t::from_page(&page);
            println!("FSP header: {fsp_header:#?}");
        }

        if page.space_id == 0 {
            self.read_trx_sys_page(&reader)?;
        }

        Ok(())
    }

    pub fn read_trx_sys_page(&self, reader: &TablespaceReader<'_>) -> anyhow::Result<()> {
        assert_eq!(reader.space_id(), 0);

        let page: PageBuf<'_> = reader.page(FSP_TRX_SYS_PAGE_NO)?;
        println!("{}", page);

        assert!(page.page_type == FIL_PAGE_TYPE_TRX_SYS);

        let trx_sys_header = trx_sys_t::from_page(&page);
        println!("{trx_sys_header:#?}");

        let undo_log_dir = self.undo_log_dir()?;

        for trx_sys_rseg_t { space_id, page_no } in trx_sys_header.rsegs {
            if space_id == reader.space_id() {
                let page: PageBuf<'_> = reader.page(page_no)?;

                self.read_sys_page(reader, &page)?;

                continue;
            }

            let new_path = undo_log_dir.join(format!("undo{:03}", space_id));

            let mmap_reader: MmapTablespaceReader =
                mdbutil::tablespace::MmapTablespaceReader::open(&new_path, self.page_size)?;
            let reader = mmap_reader.reader()?;

            let page: PageBuf<'_> = reader.page(page_no)?;
            self.read_sys_page(&reader, &page)?;
        }

        Ok(())
    }

    pub fn read_sys_page(
        &self,
        reader: &TablespaceReader<'_>,
        page: &PageBuf,
    ) -> anyhow::Result<()> {
        assert_eq!(page.page_type, FIL_PAGE_TYPE_SYS);

        println!("RSEG page: {}", page);

        let rseg = trx_rseg_t::from_page(page);

        if rseg.history_size == 0 && rseg.undo_slots.is_empty() && rseg.mysql_log.is_none() {
            if rseg.max_trx_id != 0 {
                println!("trx_rseg_t {{ max_trx_id: {} }}", rseg.max_trx_id);
                return Ok(());
            }

            return Ok(());
        }

        println!("{rseg:#?}");

        for (slot, page_no) in &rseg.undo_slots {
            let page: PageBuf<'_> = reader.page(*page_no)?;

            self.read_undo_page(reader, *slot, &page)?;
        }

        Ok(())
    }

    pub fn undo_log_dir(&self) -> anyhow::Result<PathBuf> {
        if let Some(path) = &self.undo_log_dir {
            return Ok(path.clone());
        }

        if let Some(path) = self.file_path.parent() {
            return Ok(path.to_path_buf());
        }

        Err(anyhow::anyhow!("Undo log directory not specified"))
    }

    pub fn read_undo_page(
        &self,
        _reader: &TablespaceReader<'_>,
        slot: u32,
        page: &PageBuf,
    ) -> anyhow::Result<()> {
        assert_eq!(page.page_type, FIL_PAGE_UNDO_LOG);

        println!("UNDO page (ref by slot {slot}): {}", page);

        let undo_page = trx_undo_page_t::from_page(page);
        println!("{undo_page:#?}");

        Ok(())
    }
}

impl ReadPageCommand {
    fn run(&self) -> anyhow::Result<()> {
        let file_path = &self.file_path;
        let page_size = self.page_size;

        let mmap_reader: MmapTablespaceReader =
            mdbutil::tablespace::MmapTablespaceReader::open(file_path, page_size)?;
        let num_pages = mmap_reader.mmap().len() / page_size;

        let reader: TablespaceReader<'_> = mmap_reader.reader()?;
        let page: PageBuf<'_> = reader.page(self.page)?;

        if self.hex {
            // xxd compatible hex dump
            for (i, chunk) in page.buf().chunks(16).enumerate() {
                print!("{:08x}: ", i * 16);

                for byte in chunk {
                    print!("{:02x} ", byte);
                }

                for _ in 0..(16 - chunk.len()) {
                    print!("   ");
                }

                print!("|");
                for byte in chunk {
                    if byte.is_ascii_graphic() || *byte == b' ' {
                        print!("{}", *byte as char);
                    } else {
                        print!(".");
                    }
                }
                println!("|");
            }

            return Ok(());
        }

        if self.raw {
            std::io::stdout().write_all(page.buf())?;
            return Ok(());
        }

        println!(
            "Opened tablespace file: {} with size: {} bytes, page size: {} bytes, num pages: {}, \
             flags: {}",
            file_path.display(),
            mmap_reader.mmap().len(),
            page_size,
            num_pages,
            tablespace_flags_to_string(reader.flags()),
        );

        println!("{}", reader);

        println!("{}", page);

        match page.page_type {
            FIL_PAGE_TYPE_FSP_HDR => {
                let fsp_header = fsp_header_t::from_page(&page);
                println!("FSP header: {fsp_header:#?}");
            }
            FIL_PAGE_TYPE_TRX_SYS => {
                let trx_sys_header = trx_sys_t::from_page(&page);
                println!("{trx_sys_header:#?}");
            }
            FIL_PAGE_TYPE_SYS => {
                let rseg = trx_rseg_t::from_page(&page);
                println!("{rseg:#?}");
            }
            FIL_PAGE_UNDO_LOG => {
                let undo_page = trx_undo_page_t::from_page(&page);
                println!("{undo_page:#?}");
            }
            _ => {
                return Ok(());
            }
        }

        Ok(())
    }
}
