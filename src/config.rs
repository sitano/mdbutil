use std::path::PathBuf;

use clap::Parser;

pub const LOG_FILE_NAME_PREFIX: &str = "ib_logfile";
pub const LOG_FILE_NAME: &str = "ib_logfile0";

#[derive(Parser)]
pub struct Config {
    #[clap(
        long = "log-group-path",
        help = "Path to the data directory with the log group (Redo Log)"
    )]
    pub srv_log_group_home_dir: PathBuf,
}

impl Config {
    pub fn get_log_file_path(&self) -> PathBuf {
        self.srv_log_group_home_dir.join(LOG_FILE_NAME)
    }

    pub fn get_log_file_x_path(&self, i: usize) -> PathBuf {
        self.srv_log_group_home_dir
            .join(format!("{LOG_FILE_NAME_PREFIX}{i}"))
    }
}
