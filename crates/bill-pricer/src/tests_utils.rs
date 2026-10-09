#![cfg(test)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

use chrono_tz::Tz;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;

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
