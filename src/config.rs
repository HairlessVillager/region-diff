#[cfg(not(test))]
use std::sync::OnceLock;

use crate::log::init_log;

#[derive(Clone)]
pub struct Config {
    pub log_config: LogConfig,
    pub threads: usize,
}

#[derive(Clone)]
#[allow(dead_code)]
pub enum LogConfig {
    Trace,
    Production,
    NoLog,
}

#[cfg(not(test))]
static CONFIG: OnceLock<Config> = OnceLock::new();

#[cfg(test)]
thread_local! {
    static TEST_CONFIG: std::cell::RefCell<Option<Config>> = const { std::cell::RefCell::new(None) };
}

pub fn init_config(config: Config) {
    #[cfg(not(test))]
    {
        CONFIG
            .set(config.clone())
            .unwrap_or_else(|_| panic!("cannot init config twice"));
    }

    #[cfg(test)]
    {
        TEST_CONFIG.with(|c| *c.borrow_mut() = Some(config));
    }

    init_log(&get_config().log_config);
}

#[cfg(not(test))]
pub fn get_config() -> Config {
    CONFIG.get().expect("Config not initialized").clone()
}

#[cfg(test)]
pub fn get_config() -> Config {
    TEST_CONFIG.with(|c| {
        c.borrow()
            .as_ref()
            .expect("Test config not initialized")
            .clone()
    })
}

#[cfg(test)]
pub fn with_test_config<R>(config: Config, f: impl FnOnce() -> R) -> R {
    TEST_CONFIG.with(|c| {
        *c.borrow_mut() = Some(config);
        init_log(&get_config().log_config);
        let result = f();
        *c.borrow_mut() = None;
        result
    })
}
