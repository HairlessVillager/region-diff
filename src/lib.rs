pub mod compress;
pub mod config;
pub mod diff;
pub mod logging;
pub mod mca;
pub mod util;

use std::{
    fs::{self, File},
    io::{Cursor, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    compress::CompressionType,
    config::{Config, LogConfig, init_config},
    diff::{Diff, file::MCADiff},
    util::serde::{de, ser},
};

#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// File type
    filetype: FileType,

    #[command(subcommand)]
    command: Commands,

    /// The number of threads in parallel computing
    #[arg(short, long, default_value_t = 8)]
    threads: usize,

    /// Compression type
    #[arg(short, long, default_value = "zlib")]
    compression_type: CompressionType,

    /// Use verbose output (-vv very verbose, -vvv very verbose to file)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Compare two file which have the same type
    Diff(DiffArgs),
    /// Patch the difference to the old file
    Patch(PatchArgs),
    /// Revert the difference to the new file
    Revert(RevertArgs),
    /// Squashing two adjacent differences
    Squash(SquashArgs),
}

#[derive(Debug, Args)]
struct DiffArgs {
    /// Path to old file
    old: String,
    /// Path to new file
    new: String,
    /// Path to save diff file
    diff: String,
}

#[derive(Debug, Args)]
struct PatchArgs {
    /// Path to old file
    old: String,
    /// Path to diff file
    diff: String,
    /// Path to save patched file
    patched: String,
}

#[derive(Debug, Args)]
struct RevertArgs {
    /// Path to new file
    new: String,
    /// Path to diff file
    diff: String,
    /// Path to save reverted file
    reverted: String,
}

#[derive(Debug, Args)]
struct SquashArgs {
    /// Path to base diff file
    base: String,
    /// Path to squashing diff file
    squashing: String,
    /// Path to save squashed diff file
    squashed: String,
}

#[derive(Debug, Clone, ValueEnum)]
pub enum FileType {
    /// Minecraft Region File > region/*.mca
    RegionMca,
    /// [TODO] Minecraft Region File > region/*.mcc
    RegionMcc,
}

static ERR_MSG_READ: &str = "Failed to read file";
static ERR_MSG_CREATE: &str = "Failed to create file";
static ERR_MSG_WRITE: &str = "Failed to write file";
static ERR_MSG_COMPRESS: &str = "Failed to compress data";
static ERR_MSG_DECOMPRESS: &str = "Failed to decompress data";

pub fn main() {
    let cli = Cli::parse();
    init_config(Config {
        log_config: LogConfig::Verbose(cli.verbose),
        threads: cli.threads,
    });
    log::debug!("cli args: {:#?}", cli);
    match cli.command {
        Commands::Diff(args) => {
            log::info!("reading old file...");
            let old = fs::read(PathBuf::from(args.old)).expect("cannot find old file");
            log::info!("reading new file...");
            let new = fs::read(PathBuf::from(args.new)).expect("cannot find new file");
            log::info!("comparing...");
            let diff = match cli.filetype {
                FileType::RegionMca => {
                    let diff = MCADiff::from_compare(&old, &new);
                    ser(diff)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing diff file...");
            let mut reader = Cursor::new(diff);
            let mut writer = File::create(PathBuf::from(args.diff)).expect(ERR_MSG_CREATE);
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .expect(ERR_MSG_COMPRESS);
            writer.flush().expect(ERR_MSG_WRITE);
        }
        Commands::Squash(args) => {
            log::info!("reading base diff file...");
            let base = fs::read(PathBuf::from(args.base)).expect(ERR_MSG_READ);
            let base = cli
                .compression_type
                .decompress_all(base)
                .expect(ERR_MSG_DECOMPRESS);
            log::info!("reading squashing diff file...");
            let squashing = fs::read(PathBuf::from(args.squashing)).expect(ERR_MSG_READ);
            let squashing = cli
                .compression_type
                .decompress_all(squashing)
                .expect(ERR_MSG_DECOMPRESS);
            log::info!("squashing...");
            let squashed = match cli.filetype {
                FileType::RegionMca => {
                    let base: MCADiff = de(&base);
                    let squashing: MCADiff = de(&squashing);
                    let squashed = MCADiff::from_squash(&base, &squashing);
                    ser(squashed)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing squashed diff file...");
            let mut reader = Cursor::new(squashed);
            let mut writer = File::create(PathBuf::from(args.squashed)).expect(ERR_MSG_CREATE);
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .expect(ERR_MSG_COMPRESS);
            writer.flush().expect(ERR_MSG_WRITE);
        }
        Commands::Patch(args) => {
            log::info!("reading old file...");
            let old = fs::read(PathBuf::from(args.old)).expect(ERR_MSG_READ);
            log::info!("reading diff file...");
            let diff = fs::read(PathBuf::from(args.diff)).expect(ERR_MSG_READ);
            let diff = cli
                .compression_type
                .decompress_all(diff)
                .expect(ERR_MSG_DECOMPRESS);
            log::info!("patching...");
            let patched = match cli.filetype {
                FileType::RegionMca => {
                    let diff: MCADiff = de(&diff);
                    diff.patch(&old)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing patched file...");
            let mut writer = File::create(PathBuf::from(args.patched)).expect(ERR_MSG_CREATE);
            writer.write_all(&patched).expect(ERR_MSG_WRITE);
            writer.flush().expect(ERR_MSG_WRITE);
        }
        Commands::Revert(args) => {
            log::info!("reading new file...");
            let new = fs::read(PathBuf::from(args.new)).expect(ERR_MSG_READ);
            log::info!("reading diff file...");
            let diff = fs::read(PathBuf::from(args.diff)).expect(ERR_MSG_READ);
            let diff = cli
                .compression_type
                .decompress_all(diff)
                .expect(ERR_MSG_DECOMPRESS);
            log::info!("reverting...");
            let reverted = match cli.filetype {
                FileType::RegionMca => {
                    let diff: MCADiff = de(&diff);
                    diff.revert(&new)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing reverted file...");
            let mut writer = File::create(PathBuf::from(args.reverted)).expect(ERR_MSG_CREATE);
            writer.write_all(&reverted).expect(ERR_MSG_WRITE);
            writer.flush().expect(ERR_MSG_WRITE);
        }
    }
    log::info!("success");
}
