mod commands;
mod config;
mod diff;
mod err;
mod mca;
mod object;
mod storage;
mod util;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[arg(short, long, value_name = "FILE", default_value_t = {".region-diff/config.toml".to_string()})]
    config: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize storage backend
    Init {
        #[arg(default_value_t = {".".to_string()})]
        directory: String,
    },
    /// List, create, or delete branches
    Branch {},
    /// Record changes to the storagre backend
    Commit {},
    /// Switch branches or restore working tree files
    Checkout {},
    /// Show commit logs
    Log {},
    /// Join two or more development histories together
    Merge {},
    /// Show the working tree status
    Status {},
    /// Prune all unreachable objects from the storagre backend
    Prune {},
}

fn main() {
    let cli = Cli::parse();

    // // You can check the value provided by positional arguments, or option arguments
    // if let Some(name) = cli.name.as_deref() {
    //     println!("Value for name: {name}");
    // }

    // if let Some(config_path) = cli.config.as_deref() {
    //     println!("Value for config: {}", config_path.display());
    // }

    // // You can see how many times a particular flag or argument occurred
    // // Note, only flags can have multiple occurrences
    // match cli.debug {
    //     0 => println!("Debug mode is off"),
    //     1 => println!("Debug mode is kind of on"),
    //     2 => println!("Debug mode is on"),
    //     _ => println!("Don't be crazy"),
    // }

    // You can check for the existence of subcommands, and if found use their
    // matches just as you would the top level cmd
    match &cli.command {
        Some(_) => {
            todo!()
        }
        None => {}
    }

    // Continued program logic goes here...
}
