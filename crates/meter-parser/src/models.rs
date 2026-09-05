use std::str::FromStr;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// --- Models ---

#[derive(Debug)]
pub enum ParserError {
    InvalidFormat,
    InvalidTimestamp,
    NoMeterData,
    IoError(std::io::Error),
    InvalidNumber(String),
    InvalidReadingQuality(String),
    InvalidUnitOfMeasure(String)
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
#[derive(Debug, Clone)]
pub struct MeterState {
    pub nmi: String,
    pub suffix: String,
    pub meter_type: SmartMeterType,
    pub interval_minutes: u32,
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

#[derive(Debug)]
pub struct Nem12_300 {
    pub utc_start: DateTime<Utc>,
    pub measures: Vec<Decimal>,
    pub quality: ReadingQuality,
    pub meter_type: SmartMeterType,
}
