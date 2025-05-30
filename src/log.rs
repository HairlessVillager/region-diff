use chrono::{Duration, Local, NaiveDate};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions, remove_file};
use std::io::{self, BufWriter, LineWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::config::LogConfig;

// struct Logger {
//     config: Config,
//     trace_writer: Option<Mutex<File>>,
//     debug_state: Option<Mutex<DebugState>>,
// }

struct DebugState {
    current_date: String,
    file: File,
}

fn create_debug_file(date: &str) -> io::Result<File> {
    let path = PathBuf::from("logs").join(format!("{}.log", date));
    OpenOptions::new().append(true).create(true).open(path)
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

fn cleanup_old_logs() -> io::Result<()> {
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        return Ok(());
    }

    let cutoff = Local::now().naive_local().date() - Duration::days(30);

    for entry in fs::read_dir(logs_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().unwrap_or_default() == "log" {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(date) = NaiveDate::parse_from_str(stem, "%Y%m%d") {
                    if date < cutoff {
                        remove_file(path)?;
                    }
                }
            }
        }
    }
    Ok(())
}

// impl Logger {
//     pub fn new(config: Config) -> io::Result<Self> {
//         let trace_writer = match config {
//             Config::Development => Some(Mutex::new(
//                 OpenOptions::new()
//                     .write(true)
//                     .create(true)
//                     .truncate(true)
//                     .open("dev.log")?,
//             )),
//             Config::Production => None,
//         };

//         let debug_state = match config {
//             Config::Production => {
//                 create_dir_all("logs")?;
//                 cleanup_old_logs()?;
//                 let current_date = Local::now().format("%Y%m%d").to_string();
//                 Some(Mutex::new(DebugState {
//                     current_date: current_date.clone(),
//                     file: create_debug_file(&current_date)?,
//                 }))
//             }
//             Config::Development => None,
//         };

//         Ok(Self {
//             config,
//             trace_writer,
//             debug_state,
//         })
//     }
// }

// impl Log for Logger {
//     fn enabled(&self, metadata: &Metadata) -> bool {
//         match self.config {
//             Config::Development => metadata.level() <= Level::Trace,
//             Config::Production => metadata.level() <= Level::Debug,
//         }
//     }

//     fn log(&self, record: &Record) {
//         if !self.enabled(record.metadata()) {
//             return;
//         }

//         let level = record.level();
//         let level_str = match record.level() {
//             Level::Error => "ERROR",
//             Level::Warn => "WARN",
//             Level::Info => "INFO",
//             Level::Debug => "DEBUG",
//             Level::Trace => "TRACE",
//         };

//         let now = chrono::Utc::now();
//         let timestamp = now.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string();

//         if level <= Level::Trace {
//             if let Some(writer) = &self.trace_writer {
//                 let mut guard = writer.lock().unwrap();
//                 writeln!(
//                     guard,
//                     "[{} {:<5} {}] {}",
//                     timestamp,
//                     level_str,
//                     record.module_path().unwrap_or("unknown"),
//                     record.args()
//                 )
//                 .unwrap();
//             }
//         }
//         if level <= Level::Info {
//             if let Some(state) = &self.debug_state {
//                 let today = Local::now().format("%Y%m%d").to_string();
//                 let mut guard = state.lock().unwrap();

//                 if guard.current_date != today {
//                     guard.current_date = today.clone();
//                     guard.file = create_debug_file(&today).unwrap();
//                 }

//                 writeln!(&mut guard.file, "{} - {}", record.level(), record.args()).unwrap();
//             }
//         }
//         if level <= Level::Trace {
//             println!("{} - {}", record.level(), record.args());
//         }
//     }

//     fn flush(&self) {
//         if let Some(writer) = &self.trace_writer {
//             writer.lock().unwrap().flush().unwrap();
//         }
//         if let Some(state) = &self.debug_state {
//             state.lock().unwrap().file.flush().unwrap();
//         }
//     }
// }

struct DevelopmentLogger {
    writer: Mutex<LineWriter<File>>, // 修改为 LineWriter
}

impl DevelopmentLogger {
    pub fn new() -> io::Result<Self> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open("trace.log")?;
        let writer = Mutex::new(LineWriter::new(file)); // 使用 LineWriter
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

pub fn init_log(config: &LogConfig) {
    match config {
        LogConfig::Development => {
            log::set_boxed_logger(Box::new(DevelopmentLogger::new().unwrap())).unwrap();
            log::set_max_level(LevelFilter::Trace);
        }
        LogConfig::Production => todo!("ProductionLogger"),
        LogConfig::NoLog => {}
    };
}
