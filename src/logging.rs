use chrono::{DateTime, Local};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, LineWriter, Write};
use std::sync::Mutex;

use crate::config::LogConfig;

fn now() -> DateTime<Local> {
    Local::now()
}

fn map_level_to_str(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

fn write_trace_log_file(writer: &Mutex<LineWriter<File>>, record: &Record) {
    let mut writer = writer.lock().unwrap();
    writeln!(
        writer,
        "[{:<5} {} {}] {}",
        map_level_to_str(record.level()),
        now().format("%H:%M:%S%.6f").to_string(),
        record.module_path().unwrap_or("???"),
        record.args()
    )
    .unwrap();
}

fn write_debug_log_file(writer: &Mutex<LineWriter<File>>, record: &Record) {
    let mut writer = writer.lock().unwrap();
    writeln!(
        writer,
        "[{:<5} {}] {}",
        map_level_to_str(record.level()),
        now().format("%H:%M:%S%.6f").to_string(),
        record.args()
    )
    .unwrap();
}

fn write_console_log(record: &Record) {
    eprintln!(
        "[{:<5} {}] {}",
        map_level_to_str(record.level()),
        now().format("%H:%M:%S%.3f").to_string(),
        record.args()
    )
}

mod prod {
    use super::*;

    pub struct ProductionLogger {
        writer: Option<Mutex<LineWriter<File>>>,
    }

    impl ProductionLogger {
        pub fn new(write_file: bool) -> io::Result<Self> {
            if write_file {
                let file_name = "debug.log";
                let file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(file_name)?;
                let writer = Mutex::new(LineWriter::new(file));
                Ok(Self {
                    writer: Some(writer),
                })
            } else {
                Ok(Self { writer: None })
            }
        }
    }

    impl Log for ProductionLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Debug
        }

        fn log(&self, record: &Record) {
            if let Some(writer) = &self.writer {
                write_debug_log_file(writer, record);
            }
            write_console_log(record);
        }

        fn flush(&self) {
            if let Some(writer) = &self.writer {
                let mut writer = writer.lock().unwrap();
                writer.flush().unwrap();
            }
        }
    }

    impl Drop for ProductionLogger {
        fn drop(&mut self) {
            if let Some(writer) = &mut self.writer {
                if let Ok(mut writer) = writer.lock() {
                    let _ = writer.flush();
                }
            }
        }
    }
}

mod dev {
    use super::*;

    pub struct DevelopmentLogger {
        writer: Mutex<LineWriter<File>>,
    }

    impl DevelopmentLogger {
        pub fn new() -> io::Result<Self> {
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open("trace.log")?;
            let writer = Mutex::new(LineWriter::new(file));
            Ok(Self { writer })
        }
    }

    impl Log for DevelopmentLogger {
        fn enabled(&self, _metadata: &Metadata) -> bool {
            true
        }

        fn log(&self, record: &Record) {
            write_trace_log_file(&self.writer, record);
        }

        fn flush(&self) {
            let mut writer = self.writer.lock().unwrap();
            writer.flush().unwrap();
        }
    }

    impl Drop for DevelopmentLogger {
        fn drop(&mut self) {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writer.flush();
            }
        }
    }
}
pub fn init_log(config: &LogConfig) {
    match config {
        LogConfig::Trace => {
            log::set_boxed_logger(Box::new(dev::DevelopmentLogger::new().unwrap())).unwrap();
            log::set_max_level(LevelFilter::Trace);
        }
        LogConfig::Verbose(verbose) => match *verbose {
            0 => {}
            1 => {
                let logger = prod::ProductionLogger::new(false).unwrap();
                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(LevelFilter::Info);
            }
            2 => {
                let logger = prod::ProductionLogger::new(false).unwrap();
                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(LevelFilter::Debug);
            }
            3 => {
                let logger = prod::ProductionLogger::new(true).unwrap();
                log::set_boxed_logger(Box::new(logger)).unwrap();
                log::set_max_level(LevelFilter::Debug);
            }
            4..=u8::MAX => {
                log::set_boxed_logger(Box::new(dev::DevelopmentLogger::new().unwrap())).unwrap();
                log::set_max_level(LevelFilter::Trace);
            }
        },
        LogConfig::NoLog => {}
    };
}
