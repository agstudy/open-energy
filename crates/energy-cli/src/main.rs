use bill_pricer::models::{HourMinute, InvalidStrHourMinute};
use bill_pricer::pricer::price;
use bill_pricer::tariff::{TariffFactory, Window};
use clap::{Parser, Subcommand};
use domain::meter::merge_import_export;
use rust_decimal::Decimal;
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
enum TariffTypeCmd {
    /// Flat rate tariff
    Flat {
        #[arg(long)]
        rate: Decimal,
        #[arg(long)]
        supply: Decimal,
    },
    /// Time-of-use tariff
    Tou {
        #[arg(long)]
        peak: Decimal,
        #[arg(long)]
        start_peak: String,
        #[arg(long)]
        off_peak: Decimal,
        #[arg(long)]
        start_off_peak: String,
        #[arg(long)]
        supply: Decimal,
    },
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
    /// Price NEM12 smart meter files
    Price {
        #[arg(short, long)]
        file: PathBuf,

        #[command(subcommand)]
        tariff: TariffTypeCmd,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, verbose } => {
            handle_parse(file, verbose);
        }
        Commands::Price { file, tariff } => match TariffType::try_from(tariff) {
            Ok(tariff) => handle_price(file, tariff),
            Err(e) => eprintln!("Could not parse tariff {}", e),
        },
    }
}

// --- Command Handlers ---

fn parse_file(path: &PathBuf) -> Option<Nem12Parser> {
    let file = match File::open(path) {
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

#[derive(Debug, Clone)]
enum TariffType {
    Flat {
        rate: Decimal,
        supply: Decimal,
    },
    Tou {
        peak: Window,
        off_peak: Window,
        supply: Decimal,
    },
}

impl TryFrom<TariffTypeCmd> for TariffType {
    type Error = InvalidStrHourMinute;
    fn try_from(c: TariffTypeCmd) -> Result<Self, InvalidStrHourMinute> {
        match c {
            TariffTypeCmd::Flat { rate, supply } => Ok(TariffType::Flat { rate, supply }),
            TariffTypeCmd::Tou {
                peak,
                start_peak,
                off_peak,
                start_off_peak,
                supply,
            } => Ok(TariffType::Tou {
                peak: Window {
                    rate: peak,
                    start: HourMinute::try_from(start_peak.as_str())?,
                },
                off_peak: Window {
                    rate: off_peak,
                    start: HourMinute::try_from(start_off_peak.as_str())?,
                },
                supply,
            }),
        }
    }
}

fn handle_price(path: PathBuf, tariff_type: TariffType) {
    println!("--- Pricing NEM12 smart meter ---");
    if let Some(parser) = parse_file(&path) {
        println!("Success! Parsed {} days.", parser.results.len());
        let smart_meter = merge_import_export(&parser.import(), &parser.export());

        let tariff = match tariff_type {
            TariffType::Flat { rate, supply } => TariffFactory::flat(rate, supply),
            TariffType::Tou {
                peak,
                off_peak,
                supply,
            } => TariffFactory::time_of_use(peak, off_peak, supply),
        };

        match price(&tariff, &smart_meter) {
            Ok(result) => println!("pricing result is : {:?}", result),
            Err(e) => eprintln!("Pricing failed: {:?}", e),
        }
    }
}
