use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{File, OpenOptions};
use std::io::{self, LineWriter, Write};
use std::sync::Mutex;

use crate::config::LogConfig;

fn map_level_to_str(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARN",
        Level::Info => "INFO",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

mod prod {
    use super::*;

    pub struct ProductionLogger {
        writer: Mutex<LineWriter<File>>,
    }

    impl ProductionLogger {
        pub fn new() -> io::Result<Self> {
            let file_name = "debug.log";
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(file_name)?;
            let writer = Mutex::new(LineWriter::new(file));
            Ok(Self { writer })
        }
    }

    impl Log for ProductionLogger {
        fn enabled(&self, metadata: &Metadata) -> bool {
            metadata.level() <= Level::Debug
        }

        fn log(&self, record: &Record) {
            if !self.enabled(record.metadata()) {
                return;
            }

            let level = record.level();
            let level_str = map_level_to_str(level);

            let now = chrono::Utc::now();

            let mut writer = self.writer.lock().unwrap();
            writeln!(
                writer,
                "[{} {:<5} {}] {}",
                now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
                level,
                record.module_path().unwrap_or("???"),
                record.args()
            )
            .unwrap();

            if level <= Level::Info {
                let ts = now.format("%H:%M:%S%.3f").to_string();
                eprintln!("[{} {:<5}] {}", ts, level_str, record.args())
            }
        }

        fn flush(&self) {
            let mut writer = self.writer.lock().unwrap();
            writer.flush().unwrap();
        }
    }

    impl Drop for ProductionLogger {
        fn drop(&mut self) {
            if let Ok(mut writer) = self.writer.lock() {
                let _ = writer.flush();
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
            let level = map_level_to_str(record.level());
            let ts = chrono::Utc::now().format("%H:%M:%S%.6f").to_string();

            let mut writer = self.writer.lock().unwrap();
            writeln!(
                writer,
                "[{} {:<5} {}] {}",
                ts,
                level,
                record.module_path().unwrap_or("???"),
                record.args()
            )
            .unwrap();
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
        LogConfig::Production => {
            log::set_boxed_logger(Box::new(prod::ProductionLogger::new().unwrap())).unwrap();
            log::set_max_level(LevelFilter::Debug);
        }
        LogConfig::NoLog => {}
    };
}
