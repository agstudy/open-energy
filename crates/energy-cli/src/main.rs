use bill_pricer::models::HourMinute;
use bill_pricer::pricer::price;
use bill_pricer::tariff::{Tariff, TariffRaw};
use bill_pricer::tariff_factory::{TariffFactory, Window};
use clap::{Parser, Subcommand};
use domain::meter::merge_import_export;
use rust_decimal::Decimal;
use std::fs::File;
use std::path::{Path, PathBuf};
// Import your library crates
use anyhow::Context;
use anyhow::Result as AnyResult;
use meter_parser::Nem12Parser;
use std::io::BufReader;

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
    FromFile {
        #[arg(long = "tariff-file")]
        file: PathBuf,
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



impl TryFrom<TariffCmd> for Tariff {
    type Error = anyhow::Error;

    fn try_from(c: TariffCmd) -> Result<Self, Self::Error> {
        match c {
            TariffCmd::Flat { rate, supply } => Ok(TariffFactory::flat(rate, supply)?),
            TariffCmd::Tou {
                peak,
                start_peak,
                off_peak,
                start_off_peak,
                supply,
            } => Ok(TariffFactory::time_of_use(
                Window {
                    rate: peak,
                    start: start_peak,
                },
                Window {
                    rate: off_peak,
                    start: start_off_peak,
                },
                supply,
            )?),
            TariffCmd::FromFile { file } => parse_tariff_file(&file),
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
            let bill = price_file(&file, tariff.try_into()?)?;
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

fn parse_tariff_file(path: &Path) -> AnyResult<Tariff> {
    let file = File::open(path).with_context(|| format!("Could not open {}", path.display()))?;

    let reader = BufReader::new(file);

    // Parse into generic Value
    let tariff_raw: TariffRaw = serde_json::from_reader(reader)?;

    Tariff::try_from(tariff_raw).context("Tariff serialisation failed")
}

fn price_file(path: &Path, tariff: Tariff) -> AnyResult<Decimal> {

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
