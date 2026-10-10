use chrono::{DateTime, Datelike, Duration, Timelike, Utc};

use chrono_tz::Australia::Brisbane; // The IANA timezone identifier

use csv::StringRecord;
use domain::meter::MeterMeasure;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::cmp::max;
use std::str::FromStr;
use std::{collections::BTreeMap, fs::File};

use crate::helper::{LocalConversionError, utc_from_local_str};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("frequency must be in (0, 60] and divide 60 evenly, got {0}")]
    InvalidFrequency(i64),

    #[error("system_capacity must be positive")]
    ZeroSystemCapacity,

    #[error("daily_kwh must be non-negative, got {0}")]
    NegativeDailyKwh(Decimal),
}
const AC_COL: usize = 11;
#[derive(Debug, thiserror::Error)]
pub enum PvwattError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("error to read row {0}")]
    ErrorRow(usize),
    #[error("col {0} is not found")]
    ColumnNotFound(usize),
    #[error("col {col} is not parsed because: {reason}")]
    ParseColumn { col: usize, reason: String },
    #[error("Out of range minute")]
    OutOfRange,
}

#[derive(Debug, thiserror::Error)]
pub enum GeneratorError {
    #[error("local conversion failed")]
    LocalConversion(#[from] LocalConversionError),

    #[error("Parsing pvwatt error")]
    Pvwatt(#[from] PvwattError),

    #[error("invalid generator config")]
    InvalidConfig(#[from] ConfigError),

    #[error("start {start} is not before end {end}")]
    InvalidRange { start: String, end: String },
}

pub struct GeneratorConfig {
    pub start: String,
    pub end: String,
    pub frequency: i64,
    pub daily_kwh: Decimal,
    pub system_capacity: u32,
    pub with_export: bool,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            start: "2026-01-01 00:00:00".into(),
            end: "2027-01-01 00:00:00".into(),
            frequency: 30,
            daily_kwh: dec!(24),
            with_export: false,
            system_capacity: 4,
        }
    }
}

fn validate_config(config: &GeneratorConfig) -> Result<(), ConfigError> {
    if config.frequency <= 0 || config.frequency > 60 || 60 % config.frequency != 0 {
        return Err(ConfigError::InvalidFrequency(config.frequency));
    }
    if config.system_capacity == 0 {
        return Err(ConfigError::ZeroSystemCapacity);
    }
    if config.daily_kwh < Decimal::ZERO {
        return Err(ConfigError::NegativeDailyKwh(config.daily_kwh));
    }

    Ok(())
}

pub fn generate_smart_meter(
    config: &GeneratorConfig,
    tz_str: &str,
) -> Result<Vec<MeterMeasure>, GeneratorError> {
    validate_config(config)?;

    let mut utc_start = utc_from_local_str(&config.start, tz_str)?;
    let end_utc = utc_from_local_str(&config.end, tz_str)?;

    if utc_start >= end_utc {
        return Err(GeneratorError::InvalidRange {
            start: config.start.clone(),
            end: config.end.clone(),
        });
    }

    let generated_kwhs = if config.with_export {
        Some(read_solar(config.system_capacity, config.frequency)?)
    } else {
        None
    };

    let import_per_slot = config.daily_kwh / Decimal::from(1440 / config.frequency);
    let step = Duration::minutes(config.frequency);

    let mut result = Vec::new();
    while utc_start < end_utc {
        let mut measure = MeterMeasure {
            utc_start,
            import: import_per_slot,
            export: None,
        };

        let local = measure.to_local(Brisbane).datetime;
        let key = MonthDayTime {
            month: local.month(),
            day: local.day(),
            hour: local.hour(),
            minute: local.minute(),
        };
        if let Some(kwhs) = &generated_kwhs
            && let Some(export_measure) = apply_solar(&key, utc_start, import_per_slot, kwhs)
        {
            measure = export_measure;
        }

        result.push(measure);
        utc_start += step;
    }

    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MonthDayTime {
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
}

fn apply_solar(
    key: &MonthDayTime,
    ts: DateTime<Utc>,
    import: Decimal,
    generated_kwhs: &BTreeMap<MonthDayTime, Decimal>,
) -> Option<MeterMeasure> {
    let solar = generated_kwhs.get(key);
    if let Some(value) = solar {
        let diff = import - value;
        Some(MeterMeasure {
            utc_start: ts,
            import: max(Decimal::ZERO, diff),
            export: Some(max(Decimal::ZERO, -diff)),
        })
    } else {
        None
    }
}

fn parse_column<T>(row: &StringRecord, col: usize) -> Result<T, PvwattError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    row.get(col)
        .ok_or(PvwattError::ColumnNotFound(col))?
        .trim()
        .parse::<T>()
        .map_err(|e| PvwattError::ParseColumn {
            col,
            reason: e.to_string(),
        })
}

fn scale_solar(kwh: Decimal, capacity: u32) -> Decimal {
    kwh / dec!(1000) * Decimal::from(capacity)
}
fn read_row(
    index: usize,
    record: Result<StringRecord, csv::Error>,
    capacity: u32,
) -> Result<(MonthDayTime, Decimal), PvwattError> {
    match record {
        Ok(row) => {
            let init = MonthDayTime {
                month: parse_column(&row, 0)?,
                day: parse_column(&row, 1)?,
                hour: parse_column(&row, 2)?,
                minute: 0,
            };
            let ac = scale_solar(parse_column::<Decimal>(&row, AC_COL)?, capacity);
            Ok((init, ac))
        }
        Err(_) => Err(PvwattError::ErrorRow(index)),
    }
}

fn read_solar(
    capacity: u32,
    frequency: i64,
) -> Result<BTreeMap<MonthDayTime, Decimal>, PvwattError> {
    // todo: remove this hardcoded once we create the mapping zone -> postcode -> pvwatt file
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/data/pvwatts/nsw_2000.csv");

    let reader = File::open(path)?;

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(reader);
    // skip 31 lines of header
    // todo: read header to get lat/lng and system properties

    let mut out = BTreeMap::new();

    for (i, record) in rdr.records().skip(31).enumerate() {
        let (key, value) = read_row(i, record, capacity)?;
        let samples = 60 / frequency;
        for current in 0..samples {
            let mut clone = key.clone();
            let mini64 = current * frequency;
            clone.minute = u32::try_from(mini64).map_err(|_| PvwattError::OutOfRange)?;
            out.insert(clone, value / Decimal::from(samples));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {

    use super::*;
    use domain::HourMinute;

    fn gen_meter(cap: u32, avg_kwh: u32) -> Vec<MeterMeasure> {
        generate_smart_meter(
            &GeneratorConfig {
                with_export: true,
                system_capacity: cap,
                daily_kwh: Decimal::from(avg_kwh),
                ..Default::default()
            },
            "Australia/Sydney".into(),
        )
        .unwrap()
    }

    #[test]
    fn test_pvwatt_multiple_capacities() {
        let result = read_solar(1, 60).unwrap();
        assert_eq!(
            result.iter().map(|(_, v)| v).sum::<Decimal>().floor(),
            dec!(1492)
        );
        let result = read_solar(4, 60).unwrap();
        assert_eq!(
            result.iter().map(|(_, v)| v).sum::<Decimal>().floor(),
            dec!(5970)
        );
    }
    #[test]
    fn test_smart_meter() {
        let meter = gen_meter(10, 1);
        let net_load = meter
            .iter()
            .map(|v| v.export.unwrap_or_default() - v.import)
            .sum::<Decimal>();

        assert!(net_load > Decimal::ZERO);

        let meter = gen_meter(1, 10);

        let net_load = meter
            .iter()
            .map(|v| v.export.unwrap_or_default() - v.import)
            .sum::<Decimal>();

        assert!(net_load < Decimal::ZERO);
    }

    #[test]
    fn test_read_solar_count() {
        let s60 = read_solar(1, 60).unwrap();
        let s30 = read_solar(1, 30).unwrap();
        let s15 = read_solar(1, 15).unwrap();
        assert!(s15.len() == 2 * s30.len());
        assert!(s15.len() == 4 * s60.len());
    }
    #[test]
    fn test_read_solar_split_is_continuous() {
        let s = read_solar(1, 60).unwrap();
        let s30 = read_solar(1, 30).unwrap();

        let key = MonthDayTime {
            month: 5,
            day: 15,
            hour: 11,
            minute: 0,
        };
        let key30 = MonthDayTime {
            month: 5,
            day: 15,
            hour: 11,
            minute: 30,
        };
        assert!(s.get(&key).unwrap() > &Decimal::ZERO);
        assert_eq!(
            s30.get(&key).unwrap() + s30.get(&key30).unwrap(),
            *s.get(&key).unwrap()
        )
    }

    #[test]
    fn test_solar_yearly_sum() {
        let ssum = |s: &BTreeMap<MonthDayTime, Decimal>| -> Decimal {
            s.iter().map(|(_, v)| v).sum::<Decimal>()
        };
        let s = read_solar(1, 60).unwrap();
        let s30 = read_solar(1, 30).unwrap();
        let s15 = read_solar(1, 15).unwrap();
        assert_eq!(ssum(&s), ssum(&s30));
        assert_eq!(ssum(&s), ssum(&s15));
    }
    #[test]
    fn test_midday_midnight_smart_meter() {
        let sm = generate_smart_meter(
            &GeneratorConfig {
                with_export: true,
                frequency: 30,
                system_capacity: 10,
                daily_kwh: Decimal::from(24),
                end: "2026-01-02 00:00:00".into(),

                ..Default::default()
            },
            "Australia/Sydney".into(),
        )
        .unwrap();

        let mut middday_export = sm.iter().filter(|&m| {
            let local = m.to_local(Brisbane);
            let hm1130 = HourMinute::new_unchecked(11, 30);
            let hm1230 = HourMinute::new_unchecked(12, 30);
            [hm1130, hm1230].contains(&local.hour_minute)
        });

        assert!(middday_export.all(|m| m.export.unwrap() > Decimal::ZERO));

        let mut midnight_export = sm.iter().filter(|&m| {
            let local = m.to_local(Brisbane);
            let hm030 = HourMinute::new_unchecked(0, 30);
            let hm2330 = HourMinute::new_unchecked(23, 30);
            [hm030, hm2330].contains(&local.hour_minute)
        });
        assert!(midnight_export.all(|m| m.export.unwrap() == Decimal::ZERO));
    }

    #[test]
    fn test_meter_timezone() {
        let sm = gen_meter(10, 24);
        assert_eq!(sm[0].utc_start.month(), 12);
        assert_eq!(sm[0].utc_start.hour(), 13);
        assert_eq!(sm[0].utc_start.minute(), 0);
    }
}
