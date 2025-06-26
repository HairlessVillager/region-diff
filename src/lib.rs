pub mod compress;
pub mod config;
pub mod diff;
pub mod logging;
pub mod mca;
pub mod util;

use std::{
    fs::{self, File},
    io::{self, Cursor, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    compress::CompressionType,
    config::{Config, LogConfig, init_config},
    diff::{Diff, file::MCADiff},
    util::serde::{deserialize, serialize},
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
                    serialize(diff)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing diff file...");
            let mut reader = Cursor::new(diff);
            let mut writer = File::create(PathBuf::from(args.diff)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
        Commands::Squash(args) => {
            log::info!("reading base diff file...");
            let base = fs::read(PathBuf::from(args.base)).unwrap();
            let base = cli.compression_type.decompress_all(base).unwrap();
            log::info!("reading squashing diff file...");
            let squashing = fs::read(PathBuf::from(args.squashing)).unwrap();
            let squashing = cli.compression_type.decompress_all(squashing).unwrap();
            log::info!("squashing...");
            let squashed = match cli.filetype {
                FileType::RegionMca => {
                    let base: MCADiff = deserialize(&base);
                    let squashing: MCADiff = deserialize(&squashing);
                    let squashed = MCADiff::from_squash(&base, &squashing);
                    serialize(squashed)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing squashed diff file...");
            let mut reader = Cursor::new(squashed);
            let mut writer = File::create(PathBuf::from(args.squashed)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
        Commands::Patch(args) => {
            log::info!("reading old file...");
            let old = fs::read(PathBuf::from(args.old)).unwrap();
            log::info!("reading diff file...");
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let diff = cli.compression_type.decompress_all(diff).unwrap();
            log::info!("patching...");
            let patched = match cli.filetype {
                FileType::RegionMca => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.patch(&old)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing patched file...");
            let mut reader = Cursor::new(patched);
            let mut writer = File::create(PathBuf::from(args.patched)).unwrap();
            io::copy(&mut reader, &mut writer).unwrap();
            writer.flush().unwrap();
        }
        Commands::Revert(args) => {
            log::info!("reading new file...");
            let new = fs::read(PathBuf::from(args.new)).unwrap();
            log::info!("reading diff file...");
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let diff = cli.compression_type.decompress_all(diff).unwrap();
            log::info!("reverting...");
            let reverted = match cli.filetype {
                FileType::RegionMca => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.revert(&new)
                }
                FileType::RegionMcc => todo!(),
            };
            log::info!("writing reverted file...");
            let mut reader = Cursor::new(reverted);
            let mut writer = File::create(PathBuf::from(args.reverted)).unwrap();
            io::copy(&mut reader, &mut writer).unwrap();
            writer.flush().unwrap();
        }
    }
    log::info!("success");
}
