use clap::{Parser, Subcommand};
use std::fs::File;
use std::path::PathBuf;

// Import your library crates
use meter_parser::Nem12Parser;

#[derive(Parser)]
#[command(name = "Energy Tool")]
#[command(about = "A multi-tool for energy", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse NEM12 smart meter files
    Parse {
        /// Path to the NEM12 CSV file
        #[arg(short, long)]
        file: PathBuf,

        /// Show detailed interval data
        #[arg(short, long)]
        verbose: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, verbose } => {
            handle_parse(file, verbose);
        }
    }
}

// --- Command Handlers ---

fn handle_parse(path: PathBuf, verbose: bool) {
    println!("--- Running NEM12 Parser ---");
    let file = File::open(&path).expect("Failed to open file");
    let mut parser = Nem12Parser::new();

    match parser.parse_stream(file) {
        Ok(_) => {
            println!("Success! Parsed {} days.", parser.results.len());
            if verbose {
                for result in &parser.results {
                    println!("{:?}", result);
                }
            }
            for (key, value) in &parser.summary() {
                println!("{:?}: {}", key, value);
            }
        }
        Err(e) => eprintln!("Parsing failed: {:?}", e),
    }
}
