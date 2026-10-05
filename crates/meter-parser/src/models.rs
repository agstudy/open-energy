use std::str::FromStr;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// --- Models ---

#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    #[error("Invalid format to get meter state at {0}")]
    InvalidFormat(u32),
    #[error("Invalid meter interval minutes {0}")]
    InvalidInterval(u32),
    #[error("Unknown Timestamp format")]
    InvalidTimestamp,
    #[error("No Meter state data")]
    NoMeterData,
    #[error("I/O error: {0}")]
    IoError(std::io::Error),
    #[error("Impossible to convert meter measure {0} as decimal value")]
    InvalidNumber(String),
    #[error("Unknown smart meter quality: {0} ")]
    InvalidReadingQuality(String),
    #[error("Invalid unit of measure: {0} ")]
    InvalidUnitOfMeasure(String),
    #[error("CSV error at line {line}: {source}")]
    Csv {
        line: usize,
        #[source]
        source: csv::Error,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadingQuality {
    Actual,     // A
    Substitute, // S, F
    Variable,   // V (Mix of actual and estimates)
}

impl FromStr for ReadingQuality {
    type Err = ParserError; // or your own error type

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        match code {
            "A" => Ok(ReadingQuality::Actual),
            "V" => Ok(ReadingQuality::Variable),
            "S" | "F" => Ok(ReadingQuality::Substitute),
            _ => Err(ParserError::InvalidReadingQuality(code.to_string())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnitOfMeasure {
    KiloWattHour,   // kWh
    WattHour,       // Wh
    MegaWattHour,   // MWh
    KiloVarHour,    // kvarh (Reactive)
    VoltAmpereHour, // vAh
}

impl FromStr for UnitOfMeasure {
    type Err = ParserError;
    // Converts any input string to our Enum
    fn from_str(uom: &str) -> Result<Self, Self::Err> {
        match uom.to_lowercase().as_str() {
            "kwh" => Ok(Self::KiloWattHour),
            "wh" => Ok(Self::WattHour),
            "mwh" => Ok(Self::MegaWattHour),
            "kvarh" => Ok(Self::KiloVarHour),
            "vah" => Ok(Self::VoltAmpereHour),
            _ => Err(ParserError::InvalidUnitOfMeasure(uom.to_string())),
        }
    }
}

impl UnitOfMeasure {
    // This is the "Magic" for your math
    pub fn scaling_factor(&self) -> Decimal {
        match self {
            Self::KiloWattHour => dec!(1.0),
            Self::WattHour => dec!(0.001),
            Self::MegaWattHour => dec!(1000.0),
            _ => dec!(1.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntervalMinutes(u32);

impl IntervalMinutes {
    pub fn new(v: u32) -> Result<Self, ParserError> {
        if v == 0 || !v.is_multiple_of(5) {
            return Err(ParserError::InvalidInterval(v));
        }
        Ok(Self(v))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct MeterState {
    pub nmi: String,
    pub suffix: String,
    pub meter_type: SmartMeterType,
    pub interval_minutes: IntervalMinutes,
    pub uom: UnitOfMeasure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SmartMeterType {
    KwhImport,
    KwhExport,
    KvarhImport,
    KvarhExport,
    Unknown,
}

impl std::fmt::Display for SmartMeterType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug)]
pub struct Nem12_300 {
    pub utc_start: DateTime<Utc>,
    pub measures: Vec<Decimal>,
    pub quality: ReadingQuality,
    pub meter_type: SmartMeterType,
}
