use std::io::Result;
use std::path::PathBuf;

use clap::Parser;

pub const LOG_FILE_NAME_PREFIX: &str = "ib_logfile";
pub const LOG_FILE_NAME: &str = "ib_logfile0";

#[derive(Parser)]
pub struct Config {
    // arg group
    #[clap(
        long = "log-group-path",
        help = "Path to the data directory with the log group (Redo Log)",
        group = "redo_log_file_path"
    )]
    pub srv_log_group_home_dir: Option<PathBuf>,

    #[clap(
        long = "log-file-path",
        help = "Path to the log file (Redo Log)",
        group = "redo_log_file_path"
    )]
    pub srv_log_file_path: Option<PathBuf>,

    #[clap(default_value = "false", long)]
    pub write: bool,
}

impl Config {
    pub fn get_log_file_dir(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.srv_log_group_home_dir {
            return Ok(path.clone());
        }

        if let Some(ref path) = self.srv_log_file_path {
            return Ok(path
                .parent()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Log file path does not have a parent directory",
                    )
                })?
                .to_path_buf());
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Log file directory not specified",
        ))
    }

    pub fn get_log_file_path(&self) -> Result<PathBuf> {
        if let Some(ref path) = self.srv_log_file_path {
            return Ok(path.clone());
        }

        if let Some(ref path) = self.srv_log_group_home_dir {
            return Ok(path.join(LOG_FILE_NAME));
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Log file path not specified",
        ))
    }

    pub fn get_log_file_x(i: usize) -> String {
        format!("{LOG_FILE_NAME_PREFIX}{i}")
    }
}
