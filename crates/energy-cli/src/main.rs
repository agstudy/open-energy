use bill_pricer::pricer::price;
use bill_pricer::tariff::TariffFactory;
use clap::{Parser, Subcommand};
use domain::meter::merge_import_export;
use rust_decimal_macros::dec;
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

    Price {
        #[arg(short, long)]
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, verbose } => {
            handle_parse(file, verbose);
        }
        Commands::Price { file } => {
            handle_price(file);
        }
    }
}

// --- Command Handlers ---

fn parse_file(path: &PathBuf) -> Option<Nem12Parser> {
    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Could not open {}: {}", path.display(), e);
            return None;
        }
    };

    let mut parser = Nem12Parser::new();
    match parser.parse_stream(file) {
        Ok(_) => Some(parser),
        Err(e) => {
            eprintln!("Parsing failed: {:?}", e);
            None
        }
    }
}

fn handle_parse(path: PathBuf, verbose: bool) {
    println!("--- Running NEM12 Parser ---");
    if let Some(parser) = parse_file(&path) {
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
}

fn handle_price(path: PathBuf) {
    println!("--- Pricing NEM12 smart meter ---");
    if let Some(parser) = parse_file(&path) {
        println!("Success! Parsed {} days.", parser.results.len());
        let smart_meter = merge_import_export(&parser.import(), &parser.export());

        match price(&TariffFactory::flat(dec!(0.2), dec!(1.0)), &smart_meter) {
            Ok(result) => println!("pricing result is : {:?}", result),
            Err(e) => eprintln!("Pricing failed: {:?}", e),
        }
    }
}
