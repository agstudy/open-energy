use bill_pricer::models::HourMinute;
use bill_pricer::pricer::price;
use bill_pricer::tariff::{TariffFactory, Window};
use clap::{Parser, Subcommand};
use domain::meter::merge_import_export;
use rust_decimal::Decimal;
use std::fs::File;
use std::path::{Path, PathBuf};
// Import your library crates
use anyhow::Context;
use anyhow::Result as AnyResult;
use meter_parser::Nem12Parser;

#[derive(Parser)]
#[command(name = "Energy Tool")]
#[command(about = "A multi-tool for energy", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum TariffCmd {
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
        start_peak: HourMinute,
        #[arg(long)]
        off_peak: Decimal,
        #[arg(long)]
        start_off_peak: HourMinute,
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
        tariff: TariffCmd,
    },
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

impl From<TariffCmd> for TariffType {
    fn from(c: TariffCmd) -> Self {
        match c {
            TariffCmd::Flat { rate, supply } => TariffType::Flat { rate, supply },
            TariffCmd::Tou {
                peak,
                start_peak,
                off_peak,
                start_off_peak,
                supply,
            } => TariffType::Tou {
                peak: Window {
                    rate: peak,
                    start: start_peak,
                },
                off_peak: Window {
                    rate: off_peak,
                    start: start_off_peak,
                },
                supply,
            },
        }
    }
}

fn main() -> AnyResult<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Parse { file, verbose } => {
            run_parse(&file, verbose)?;
        }
        Commands::Price { file, tariff } => {
            let bill = price_file(&file, tariff.into())?;
            println!("Bill is : {}", bill);
        }
    }
    Ok(())
}

// --- Command Handlers ---

fn parse_file(path: &Path) -> AnyResult<Nem12Parser> {
    let file = File::open(path).with_context(|| format!("Could not open {}", path.display()))?;

    let mut parser = Nem12Parser::new();
    parser.parse_stream(file).context("NEM12 Parsing failed")?;
    Ok(parser)
}

fn price_file(path: &Path, tariff_type: TariffType) -> AnyResult<Decimal> {
    let tariff = match tariff_type {
        TariffType::Flat { rate, supply } => TariffFactory::flat(rate, supply),
        TariffType::Tou {
            peak,
            off_peak,
            supply,
        } => TariffFactory::time_of_use(peak, off_peak, supply)?,
    };

    eprintln!("--- Pricing NEM12 smart meter ---");
    let parser = parse_file(path).context("Failed to parse Nem12 file")?;
    eprintln!("Success! Parsed {} days.", parser.results.len());
    let smart_meter = merge_import_export(&parser.import(), &parser.export());

    Ok(price(&tariff, &smart_meter)?)
}

fn run_parse(path: &Path, verbose: bool) -> AnyResult<()> {
    eprintln!("--- Running NEM12 Parser ---");
    let parser = parse_file(path)?;
    eprintln!("Success! Parsed {} days.", parser.results.len());
    if verbose {
        for result in &parser.results {
            println!("{:#?}", result);
        }
    }
    for (key, value) in &parser.summary() {
        println!("{}: {}", key, value);
    }
    Ok(())
}
