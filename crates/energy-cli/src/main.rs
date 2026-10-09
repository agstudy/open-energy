use anyhow::Context;
use anyhow::Result as AnyResult;
use bill_pricer::pricer::price;
use bill_pricer::tariff::{Tariff, TariffRaw};
use bill_pricer::tariff_factory::{TariffFactory, Window};
use chrono_tz::Tz;
use clap::{Args, Parser, Subcommand};
use domain::HourMinute;
use domain::meter::{MeterMeasure, PricingInput, merge_import_export};
use gen_meter::generator::{GeneratorConfig, generate_smart_meter};
use meter_parser::Nem12Parser;
use rust_decimal::Decimal;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

#[derive(Args)]
struct GeneratedArgs {
    #[arg(long, requires = "generated")]
    start: Option<String>,
    #[arg(long, requires = "generated")]
    end: Option<String>,
    #[arg(long, requires = "generated")]
    frequency: Option<i64>,
    #[arg(long, requires = "generated")]
    daily_kwh: Option<Decimal>,
    #[arg(long, requires_all = ["generated","with_export"])]
    system_capacity: Option<u32>,
    #[arg(long, requires = "generated")]
    with_export: bool,
}

impl GeneratedArgs {
    fn into_config(self) -> GeneratorConfig {
        let defaults = GeneratorConfig::default();
        GeneratorConfig {
            start: self.start.unwrap_or(defaults.start),
            end: self.end.unwrap_or(defaults.end),
            frequency: self.frequency.unwrap_or(defaults.frequency),
            daily_kwh: self.daily_kwh.unwrap_or(defaults.daily_kwh),
            system_capacity: self.system_capacity.unwrap_or(defaults.system_capacity),
            with_export: self.with_export || defaults.with_export,
        }
    }
}

#[derive(Args)]
#[command(group(
    clap::ArgGroup::new("source")
        .required(true)
        .multiple(false)
))]
struct SourceArgs {
    #[arg(short, long, group = "source")]
    file: Option<PathBuf>,

    #[arg(long, group = "source")] // optional
    generated: bool,

    #[command(flatten)]
    genn: GeneratedArgs,
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
        #[command(flatten)]
        source: SourceArgs,

        #[command(subcommand)]
        tariff: TariffCmd,

        #[arg(long)]
        tz: String,
    },
}

impl SourceArgs {
    fn into_measures(self, tz_str: &str) -> AnyResult<Vec<MeterMeasure>> {
        match (self.file, self.generated) {
            (Some(path), _) => {
                eprintln!("--- Pricing NEM12 smart meter ---");
                let parser = parse_file(&path).context("Failed to parse Nem12 file")?;
                Ok(merge_import_export(&parser.import(), &parser.export()))
            }
            (_, true) => {
                let cfg = self.genn.into_config();
                Ok(generate_smart_meter(&cfg, tz_str)?)
            }
            _ => unreachable!("clap ArgGroup guarantees exactly one source"),
        }
    }
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
                &Window {
                    rate: peak,
                    start: start_peak,
                },
                &Window {
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
        Commands::Price { source, tariff, tz } => {
            let bill = run_price(source, tariff, &tz)?;
            println!("bill is {}", bill.round_dp(2));
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

    let tariff_raw: TariffRaw = serde_json::from_reader(reader)?;

    Tariff::try_from(tariff_raw).context("Failed to load tariff")
}

fn run_parse(path: &Path, verbose: bool) -> AnyResult<()> {
    eprintln!("--- Parsing NEM12 smart meter ---");
    let parser = parse_file(path)?;
    eprintln!("Success! Parsed {} days.", parser.results.len());
    if verbose {
        for result in &parser.results {
            println!("{result:#?}");
        }
    }
    for (key, value) in &parser.summary() {
        println!("{key}: {value}");
    }
    Ok(())
}

fn run_price(source: SourceArgs, tariff: TariffCmd, tz: &str) -> AnyResult<Decimal>{
    eprintln!("--- Pricing smart meter ---");

    let tariff = tariff.try_into()?;
    let smart_meter = source.into_measures(tz)?;
    let pricing_input = PricingInput::new(Tz::from_str(tz)?, &smart_meter);
    Ok(price(&tariff, &pricing_input)?)
}
