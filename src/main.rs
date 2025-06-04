mod compress;
mod config;
mod diff;
mod log;
mod mca;
mod util;

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
    util::serde::{deserialize, serialize},
};

#[derive(Parser)]
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
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two file which have the same type
    Diff(DiffArgs),
    /// Patch the difference to a old file
    Patch(PatchArgs),
    /// Revert the difference to a new file
    Revert(RevertArgs),
    /// Squashing two adjacent differences
    Squash(SquashArgs),
}

#[derive(Args)]
struct DiffArgs {
    /// Path to old file
    old: String,
    /// Path to new file
    new: String,
    /// Path to save diff file
    diff: String,
}

#[derive(Args)]
struct PatchArgs {
    /// Path to old file
    old: String,
    /// Path to diff file
    diff: String,
    /// Path to save patched file
    patched: String,
}

#[derive(Args)]
struct RevertArgs {
    /// Path to new file
    new: String,
    /// Path to diff file
    diff: String,
    /// Path to save reverted file
    reverted: String,
}

#[derive(Args)]
struct SquashArgs {
    /// Path to base diff file
    base: String,
    /// Path to squashing diff file
    squashing: String,
    /// Path to save squashed diff file
    squashed: String,
}

#[derive(Clone, ValueEnum)]
enum FileType {
    /// Minecraft region file
    Region,
}

fn main() {
    let cli = Cli::parse();
    init_config(Config {
        log_config: LogConfig::Production,
        threads: cli.threads,
    });
    match cli.command {
        Commands::Diff(args) => {
            let old = fs::read(PathBuf::from(args.old)).expect("cannot find old file");
            let new = fs::read(PathBuf::from(args.new)).expect("cannot find new file");
            let diff = match cli.filetype {
                FileType::Region => {
                    let diff = MCADiff::from_compare(&old, &new);
                    serialize(diff)
                }
            };
            let mut reader = Cursor::new(diff);
            let mut writer = File::create(PathBuf::from(args.diff)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
        Commands::Squash(args) => {
            let base = fs::read(PathBuf::from(args.base)).unwrap();
            let base = cli.compression_type.decompress_all(base).unwrap();
            let squashing = fs::read(PathBuf::from(args.squashing)).unwrap();
            let squashing = cli.compression_type.decompress_all(squashing).unwrap();
            let squashed = match cli.filetype {
                FileType::Region => {
                    let base: MCADiff = deserialize(&base);
                    let squashing: MCADiff = deserialize(&squashing);
                    let squashed = MCADiff::from_squash(&base, &squashing);
                    serialize(squashed)
                }
            };
            let mut reader = Cursor::new(squashed);
            let mut writer = File::create(PathBuf::from(args.squashed)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
        Commands::Patch(args) => {
            let old = fs::read(PathBuf::from(args.old)).unwrap();
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let diff = cli.compression_type.decompress_all(diff).unwrap();
            let patched = match cli.filetype {
                FileType::Region => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.patch(&old)
                }
            };
            let mut reader = Cursor::new(patched);
            let mut writer = File::create(PathBuf::from(args.patched)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
        Commands::Revert(args) => {
            let new = fs::read(PathBuf::from(args.new)).unwrap();
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let diff = cli.compression_type.decompress_all(diff).unwrap();
            let reverted = match cli.filetype {
                FileType::Region => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.revert(&new)
                }
            };
            let mut reader = Cursor::new(reverted);
            let mut writer = File::create(PathBuf::from(args.reverted)).unwrap();
            cli.compression_type
                .compress(&mut reader, &mut writer)
                .unwrap();
            writer.flush().unwrap();
        }
    }
}
