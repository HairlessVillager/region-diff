mod config;
mod diff;
mod log;
mod mca;
mod util;

use std::{
    fs,
    io::{self, BufWriter, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    config::{Config, LogConfig, init_config},
    diff::{Diff, file::MCADiff},
    util::serde::{deserialize, serialize},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long)]
    threads: usize,
}

#[derive(Subcommand)]
enum Commands {
    /// Compare two file which have the same type, and output a difference
    Diff(DiffArgs),
    /// Apply the difference to a old file as patching to output the new file
    Patch(PatchRevertArgs),
    /// Apply the difference to a new file as reverting to output the old file
    Revert(PatchRevertArgs),
    /// Squashing two adjacent differences
    Squash(SquashArgs),
}

#[derive(Args)]
struct DiffArgs {
    /// File type
    filetype: FileType,
    /// Path to old file
    old: String,
    /// Path to new file
    new: String,
}

#[derive(Args)]
struct PatchRevertArgs {
    /// File type
    filetype: FileType,
    /// Path to base file
    base: String,
    /// Path to diff file
    diff: String,
}

#[derive(Args)]
struct SquashArgs {
    /// File type
    filetype: FileType,
    /// Path to base diff file
    base: String,
    /// Path to squashing diff file
    squashing: String,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
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
            let old = fs::read(PathBuf::from(args.old)).unwrap();
            let new = fs::read(PathBuf::from(args.new)).unwrap();
            let ser = match args.filetype {
                FileType::Region => {
                    let diff = MCADiff::from_compare(&old, &new);
                    serialize(diff)
                }
            };
            let mut writer = BufWriter::new(io::stdout().lock());
            writer.write_all(&ser).unwrap();
            writer.flush().unwrap();
        }
        Commands::Patch(args) => {
            let old = fs::read(PathBuf::from(args.base)).unwrap();
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let new = match args.filetype {
                FileType::Region => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.patch(&old)
                }
            };
            let mut writer = BufWriter::new(io::stdout().lock());
            writer.write_all(&new).unwrap();
            writer.flush().unwrap();
        }
        Commands::Revert(args) => {
            let new = fs::read(PathBuf::from(args.base)).unwrap();
            let diff = fs::read(PathBuf::from(args.diff)).unwrap();
            let old = match args.filetype {
                FileType::Region => {
                    let diff: MCADiff = deserialize(&diff);
                    diff.revert(&new)
                }
            };
            let mut writer = BufWriter::new(io::stdout().lock());
            writer.write_all(&old).unwrap();
            writer.flush().unwrap();
        }
        Commands::Squash(args) => {
            let base = fs::read(PathBuf::from(args.base)).unwrap();
            let squashing = fs::read(PathBuf::from(args.squashing)).unwrap();
            let ser = match args.filetype {
                FileType::Region => {
                    let base: MCADiff = deserialize(&base);
                    let squashing: MCADiff = deserialize(&squashing);
                    let squashed = MCADiff::from_squash(&base, &squashing);
                    serialize(squashed)
                }
            };
            let mut writer = BufWriter::new(io::stdout().lock());
            writer.write_all(&ser).unwrap();
            writer.flush().unwrap();
        }
    }
}
