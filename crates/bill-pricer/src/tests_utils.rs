#![cfg(test)]

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use domain::meter::MeterMeasure;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;
use std::{collections::BTreeMap, fs::File};

use crate::{
    tariff::Tariff,
    tariff_factory::{TariffFactory, Window},
};

pub fn utc_from_local(local: NaiveDateTime, tz_str: &str) -> DateTime<Utc> {
    let timezone = Tz::from_str(tz_str).expect(&format!("invalid time zone {}", tz_str));
    timezone
        .from_local_datetime(&local)
        .single()
        .expect(&format!(
            "Ambiguous or nonexistent local time {} in timezone {}",
            local, tz_str
        ))
        .with_timezone(&Utc)
}

pub fn str_to_native_datetime(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("Failed to parse naive datetime")
}

pub fn str_to_native_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("Failed to parse naive date")
}

pub fn get_tou_tariff(
    peak: Decimal,
    off_peak: Decimal,
    start_peak: &str,
    start_off_peak: &str,
) -> Tariff {
    TariffFactory::time_of_use(
        &Window {
            rate: peak,
            start: start_peak.try_into().unwrap(),
        },
        &Window {
            rate: off_peak,
            start: start_off_peak.try_into().unwrap(),
        },
        dec!(1.0),
    )
    .unwrap()
}

pub struct GeneratorConfig {
    pub start: String,
    pub end: String,
    pub frequency: i64,
    pub timezone: String,
    pub daily_kwh: Decimal,
    pub solar_year: u32,
    pub system_capacity: u32,
    pub with_export: bool,
}

impl Default for GeneratorConfig {
    fn default() -> Self {
        Self {
            start: "2026-01-01 00:00:00".into(),
            end: "2027-01-01 00:00:00".into(),
            frequency: 30,
            timezone: "Australia/Sydney".into(),
            daily_kwh: dec!(24),
            solar_year: 2026,
            with_export: false,
            system_capacity: 4,
        }
    }
}
pub fn generate_smart_meter(config: &GeneratorConfig) -> Vec<MeterMeasure> {
    let mut start_utc = utc_from_local(
        str_to_native_datetime(config.start.as_str()),
        config.timezone.as_str(),
    );
    let end_utc = utc_from_local(
        str_to_native_datetime(config.end.as_str()),
        config.timezone.as_str(),
    );
    if config.with_export {
        let mut import: Vec<(DateTime<Utc>, Decimal)> = vec![];
        while start_utc < end_utc {
            let value = (start_utc, config.daily_kwh / Decimal::from(1440 / 60));
            import.push(value);
            start_utc += Duration::minutes(60)
        }
        let export = read_solar(config.solar_year, config.system_capacity).unwrap();
        let result = consume_solar(&import, &export);

        result
    } else {
        let mut result: Vec<MeterMeasure> = vec![];
        while start_utc < end_utc {
            let measure = MeterMeasure {
                utc_start: start_utc,
                import: config.daily_kwh / Decimal::from(1440 / config.frequency),
                export: None,
            };
            result.push(measure);

            start_utc += Duration::minutes(config.frequency)
        }
        result
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct DateNoYear {
    hour: u32,
    minute: u32,
    day: u32,
    month: u32,
}

fn consume_solar(
    import: &[(DateTime<Utc>, Decimal)],
    generated_ac: &[(DateTime<Utc>, Decimal)],
) -> Vec<MeterMeasure> {
    let mut import_tree: BTreeMap<DateNoYear, MeterMeasure> = BTreeMap::new();

    for (ts, val) in import {
        let key = DateNoYear {
            hour: ts.hour(),
            minute: ts.minute(),
            day: ts.day(),
            month: ts.month(),
        };

        let vh = import_tree
            .entry(key)
            .or_insert(MeterMeasure::new(*ts, Decimal::ZERO));
        vh.import += val;
    }

    for &(ts, value) in generated_ac {
        let key = DateNoYear {
            hour: ts.hour(),
            minute: ts.minute(),
            day: ts.day(),
            month: ts.month(),
        };

        import_tree.entry(key).and_modify(|m| {
            if value < m.import {
                m.import -= value;
                m.export = Some(Decimal::ZERO);
            } else {
                m.import = Decimal::ZERO;
                m.export = Some(value - m.export.unwrap_or_default());
            };
        });
    }

    import_tree.into_values().collect()
}

fn read_solar(
    year: u32,
    capacity: u32,
) -> Result<Vec<(DateTime<Utc>, Decimal)>, Box<dyn std::error::Error>> {
    let path = "../../data/raw/pvwatts/nsw_2000.csv";
    let reader = File::open(path).unwrap();

    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(reader);
    // skip 31 lines of header
    // todo: read header to get lat/lng and system properties

    let mut out = Vec::with_capacity(8760);

    for (i, result) in rdr.records().skip(31).enumerate() {
        let row = result?;
        let month = row.get(0).unwrap().trim().parse()?;
        let day = row.get(1).unwrap().trim().parse()?;
        let hour = row.get(2).unwrap().trim().parse()?; // 0..=23
        let ac = (row.get(11).unwrap().trim().parse::<Decimal>()?) / dec!(1000)
            * Decimal::from(capacity);

        let start_utc: DateTime<Utc> = Utc
            .with_ymd_and_hms(year as i32, month, day, hour, 0, 0)
            .single()
            .ok_or_else(|| format!("row {i}: invalid date/time"))?;

        // W -> kWh for the hour (/ 1000)
        // *capacity -> scaling approx starting from 1KWH system
        out.push((start_utc, ac.round_dp(2)));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_pvwatt() {
        let result = read_solar(2026, 1).unwrap();
        assert_eq!(
            result.iter().map(|(_, v)| v).sum::<Decimal>().floor(),
            dec!(1492)
        );
        let result = read_solar(2026, 4).unwrap();
        assert_eq!(
            result.iter().map(|(_, v)| v).sum::<Decimal>().floor(),
            dec!(5970)
        );
    }
    #[test]
    fn test_smart_meter() {
        let meter = generate_smart_meter(&GeneratorConfig {
            with_export: true,
            system_capacity: 10,
            daily_kwh: dec!(2),
            ..Default::default()
        });
        // todo : change this to 8760
        assert_eq!(meter.len(), 24 * 365);
    }
}
