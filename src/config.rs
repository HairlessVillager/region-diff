use std::path::PathBuf;
use std::sync::OnceLock;

use crate::log::init_log;

pub struct Config {
    pub(crate) backend_url: String,
    pub(crate) base_dir: PathBuf,
    pub(crate) working_dir: PathBuf,
    pub(crate) log_config: LogConfig,
}

pub enum LogConfig {
    Development,
    Production,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn init_config(config: Config) {
    CONFIG.set(config).unwrap_or_else(|_| {
        panic!("cannot init config again after init");
    });
    init_log(&get_config().log_config);
}

pub fn get_config() -> &'static Config {
    CONFIG.get().expect("cannot get config before init")
}
