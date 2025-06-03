use std::{
    fs,
    io::{self, BufWriter, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    diff::{Diff, file::MCADiff},
    util::serde::{deserialize, serialize},
};

mod config;
mod diff;
mod log;
mod mca;
mod util;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    Diff(DiffArgs),
    Patch(PatchRevertArgs),
    Revert(PatchRevertArgs),
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
    /// Output mode
    #[arg(value_enum)]
    mode: Mode,
}
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Mode {
    /// Output as hex string to stdout
    Hex,
    /// Output as raw data to stdout
    Raw,
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
    /// Output mode
    #[arg(value_enum)]
    mode: Mode,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FileType {
    /// Minecraft region file
    Region,
}

fn write_hex<W, D>(writer: &mut W, data: &D, mode: Mode)
where
    W: Write,
    D: AsRef<[u8]>,
{
    match mode {
        Mode::Raw => {
            writer.write_all(data.as_ref()).unwrap();
        }
        Mode::Hex => {
            for line in data.as_ref().chunks(16) {
                for byte in line {
                    writer
                        .write_all(format!("{:02x} ", byte).as_bytes())
                        .unwrap();
                }
                writer.write_all("\n".as_bytes()).unwrap();
            }
        }
    }
}
fn main() {
    let cli = Cli::parse();
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
            write_hex(&mut writer, &ser, args.mode);
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
            write_hex(&mut writer, &ser, args.mode);
            writer.flush().unwrap();
        }
    }
}
