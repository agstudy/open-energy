use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::str::FromStr;

use crate::models::{
    MeterState, Nem12_300, ParserError, ReadingQuality, SmartMeterType, UnitOfMeasure,
};
use crate::utils::parse_date;

// --- Implementation ---
const MINUTES_PER_DAY: u32 = 24 * 60;

pub struct Nem12Parser {
    pub current_meter: Option<MeterState>,
    pub results: Vec<Nem12_300>,
}

fn parse_200_to_state(row: &csv::StringRecord) -> Result<MeterState, ParserError> {
    let nmi = row.get(1).ok_or(ParserError::InvalidFormat)?.to_string();
    let suffix = row.get(4).ok_or(ParserError::InvalidFormat)?.to_string();
    let uom_str = row.get(7).ok_or(ParserError::InvalidFormat)?;
    let uom = UnitOfMeasure::from_str(uom_str)?;

    let interval_minutes_str = row.get(8).ok_or(ParserError::InvalidFormat)?;
    let interval_minutes = interval_minutes_str
        .parse::<u32>()
        .map_err(|_| ParserError::InvalidFormat)?;

    // Determine the type based on the first character of the suffix
    // and the Unit of Measure (UOM)

    let first_char = suffix.chars().next().unwrap_or(' ');

    let meter_type = match (first_char, uom) {
        // Active Energy (kWh)
        ('E', UnitOfMeasure::KiloWattHour) | ('N', UnitOfMeasure::KiloWattHour) => {
            SmartMeterType::KwhImport
        }
        ('B', UnitOfMeasure::KiloWattHour) => SmartMeterType::KwhExport,

        // Reactive Energy (kVARh)
        ('Q', UnitOfMeasure::KiloVarHour) => SmartMeterType::KvarhImport,
        ('K', UnitOfMeasure::KiloVarHour) => SmartMeterType::KvarhExport,

        // Fallback for cases where UOM is WattHours but suffix is E/B
        ('E', UnitOfMeasure::WattHour) => SmartMeterType::KwhImport,
        ('B', UnitOfMeasure::WattHour) => SmartMeterType::KwhExport,

        _ => SmartMeterType::Unknown,
    };
    Ok(MeterState {
        nmi,
        suffix,
        meter_type,
        interval_minutes,
        uom,
    })
}

impl Nem12Parser {
    pub fn new() -> Self {
        Nem12Parser {
            current_meter: None,
            results: vec![],
        }
    }

    pub fn handle_record(&mut self, record: &csv::StringRecord) -> Result<(), ParserError> {
        match record.get(0) {
            Some("200") => {
                self.current_meter = Some(parse_200_to_state(record)?);
                Ok(())
            }
            Some("300") => {
                let data = self.transform_300(record)?;
                self.results.push(data);

                Ok(())
            }

            _ => Ok(()),
        }
    }

    /// Streams the file line-by-line.
    /// This is where the IO concern lives.
    pub fn parse_stream<R: std::io::Read>(&mut self, reader: R) -> Result<(), ParserError> {
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .flexible(true)
            .from_reader(reader); // Takes any reader (File, Cursor, Network)

        for (index, result) in rdr.records().enumerate() {
            let record = result.map_err(|_| ParserError::InvalidFormat)?;
            if let Err(e) = self.handle_record(&record) {
                eprintln!("Row {}: Parsing Error: {:?}", index + 1, e);
                // We do NOT return Err here, so the loop continues
            }
        }
        Ok(())
    }

    fn transform_300(&mut self, record: &csv::StringRecord) -> Result<Nem12_300, ParserError> {
        // Get the current meter state, error if missing
        let state = self
            .current_meter
            .as_ref()
            .ok_or(ParserError::NoMeterData)?;

        let date_str = record.get(1).ok_or(ParserError::InvalidFormat)?;

        let utc_start = parse_date(date_str)?;

        // 1. Calculate how many values we need
        let expected_count = (MINUTES_PER_DAY / state.interval_minutes) as usize;

        // 2. Identify the Quality code index if it exists in the "Standard" position
        // record.len() >= expected_count + 7 implies index (len - 5) is safe.
        let quality_code = record
            .get(record.len().saturating_sub(5))
            .filter(|_| record.len() >= expected_count + 7)
            .unwrap_or("A");

        let quality = ReadingQuality::from_str(quality_code)?;
        let values_to_take = expected_count;

        let factor = state.uom.scaling_factor();
        let values: Vec<Decimal> = record
            .iter()
            .skip(2)
            .take(values_to_take)
            .map(|v| {
                v.parse::<Decimal>()
                    .map_err(|_| ParserError::InvalidNumber(v.to_string()))
                    .map(|d| d * factor)
            })
            .collect::<Result<Vec<Decimal>, ParserError>>()?;
        let meter_type = state.meter_type;
        Ok(Nem12_300 {
            utc_start,
            measures: values,
            quality,
            meter_type,
        })
    }

    pub fn summary(&self) -> HashMap<SmartMeterType, Decimal> {
        self.results.iter().fold(HashMap::new(), |mut acc, row| {
            // .iter().sum() works on Vec<Decimal> thanks to rust_decimal
            let row_total: Decimal = row.measures.iter().sum();

            // Update the running total for this specific meter type
            *acc.entry(row.meter_type).or_insert(dec!(0.0)) += row_total;

            acc
        })
    }
}

impl Default for Nem12Parser {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nmi_200_parsing() {
        // Your exact example
        let raw_line = "200,6203230232,E1E2,E1,E1,,A7696039,KWH,30,";
        let record = csv::StringRecord::from(raw_line.split(',').collect::<Vec<_>>());

        let state = parse_200_to_state(&record).expect("Should parse valid 200");

        assert_eq!(state.nmi, "6203230232");
        assert_eq!(state.uom, UnitOfMeasure::KiloWattHour);
        assert_eq!(state.interval_minutes, 30);
    }
    #[test]
    fn test_streaming_from_memory() {
        let data = "200,6203230232,E1E2,E1,E1,,A7696039,KWH,30,
300,20230602,0.8020,0.2530,0.2390,0.2500,0.2560,0.2650,0.3010,0.2680,0.2100,0.1770,0.2150,0.2780,0.2350,0.2250,0.2740,0.3080,0.3650,0.3310,0.3010,0.4830,0.2280,0.2860,0.2730,0.2040,0.1650,0.1550,0.1540,0.1530,0.1650,0.1650,0.1670,0.2130,0.3310,0.3260,0.3250,0.3550,0.8870,0.4660,0.3860,0.5690,1.9950,1.7580,0.5160,0.4750,0.4730,0.4750,0.2790,0.1420,A,,,20230608114734,
300,20230603,0.3140,0.3030,0.2490,0.2450,0.1710,0.2200,0.1590,0.1520,0.1520,0.1510,0.1530,0.2410,0.2650,0.2450,0.1960,0.1740,0.2310,0.4950,0.5170,1.0500,0.3870,0.3900,0.5270,0.1530,0.1510,0.1530,0.1910,0.2430,0.1990,0.1680,0.1580,0.1570,0.1490,0.1510,0.1600,0.1500,0.1530,0.1550,0.1560,0.1580,0.1610,0.1530,0.1520,0.1530,0.1520,0.1520,0.1530,0.1520,A,,,20230604014253,
";
        let mut parser = Nem12Parser::new();

        // Use a Cursor to simulate a file in memory
        parser.parse_stream(std::io::Cursor::new(data)).unwrap();

        assert_eq!(parser.results.len(), 2);
    }
    #[test]
    fn test_legacy() {
        let data = "200,540423685,E1B1,E1,E1,N1,2734.258,KWH,60,,,,,,,,,,,,,,,,,
300,1/01/2024,0.271,0.284,0.237,0.223,0.198,0.177,0.635,0.029,0.035,0.019,0,0,0,0.065,0,0.012,0.091,0.891,0.528,1.29,0.961,0.433,0.554,0.176";
        let mut parser = Nem12Parser::new();
        parser.parse_stream(std::io::Cursor::new(data)).unwrap();

        assert_eq!(parser.results.len(), 1);
        // Use if let to access the inner fields safely
        if let Some(meter) = &parser.current_meter {
            assert_eq!(meter.uom, UnitOfMeasure::KiloWattHour);
            assert_eq!(meter.interval_minutes, 60);
            assert_eq!(meter.nmi, "540423685");
        } else {
            panic!("MeterState was None!");
        }

        let last_value = parser.results[0]
            .measures
            .last()
            .expect("The measures vector should not be empty");
        assert_eq!(*last_value, dec!(0.176));
    }

    #[test]
    fn test_origin() {
        let data = "100,NEM12,2.02304E+11,MDP1,Origin,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,
200,6102042981,B1E1K1Q1,E1,E1,N1,PED021601270,Kwh,15,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,,
300,20220411,2.88,2.876,3.516,2.912,3.084,2.876,3.472,2.876,2.824,2.856,2.856,3.648,2.88,2.88,2.88,2.828,2.84,2.816,3.06,2.864,2.848,2.832,2.816,2.892,2.848,3.084,3.2,3.224,5.84,6.568,6.348,6.768,6.272,6.404,8.164,8.16,8.156,8.728,8.804,9.564,10.172,10.588,9.26,9.66,9.94,8.22,9.42,10.496,9.544,10.312,8.712,9.248,9.84,9.412,9.888,8.424,9.772,9.024,8.436,9.816,9.736,8.676,9.488,9.532,9.1,9.06,7.396,7.024,6.66,3.256,2.568,2.688,2.704,2.776,2.792,2.976,2.796,2.784,2.796,2.86,4.544,4.724,4.952,4.752,4.356,2.796,2.784,2.876,4.512,4.884,4.764,4.74,4.74,4.776,4.776,4.816,A,,,2.02204E+13
";

        let mut parser = Nem12Parser::new();
        parser.parse_stream(std::io::Cursor::new(data)).unwrap();

        let last_value = parser.results[0]
            .measures
            .last()
            .expect("The measures vector should not be empty");
        assert_eq!(*last_value, dec!(4.816));
    }
}
