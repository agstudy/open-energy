#![cfg(test)]

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};

use chrono_tz::Tz;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::str::FromStr;

use crate::{
    tariff::{Tariff, TariffError},
    tariff_factory::{TariffFactory, Window},
};
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn utc_from_local(local: NaiveDateTime, tz_str: &str) -> DateTime<Utc> {
    let timezone = Tz::from_str(tz_str).unwrap_or_else(|_| panic!("invalid time zone {tz_str}"));
    timezone
        .from_local_datetime(&local)
        .single()
        .unwrap_or_else(|| {
            panic!("Ambiguous or nonexistent local time {local} in timezone {tz_str}")
        })
        .with_timezone(&Utc)
}

#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn str_to_native_datetime(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").expect("Failed to parse naive datetime")
}
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn str_to_native_date(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("Failed to parse naive date")
}
/// Builds a single-peak time-of-use [`Tariff`] from a peak and off-peak
/// rate, each with its own start time.
///
/// # Errors
///
/// Returns [`TariffError`] if either start time fails to parse, or if
/// [`TariffFactory::time_of_use`] rejects the resulting bands (e.g. they
/// overlap or leave a gap).
pub fn get_tou_tariff(
    peak: Decimal,
    off_peak: Decimal,
    start_peak: &str,
    start_off_peak: &str,
) -> Result<Tariff, TariffError> {
    TariffFactory::time_of_use(
        &Window {
            rate: peak,
            start: start_peak.try_into()?,
        },
        &Window {
            rate: off_peak,
            start: start_off_peak.try_into()?,
        },
        dec!(1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYDNEY: &str = "Australia/Sydney";
    const BRISBANE: &str = "Australia/Brisbane";
    fn utc_local(local: &str, zone: &str) -> chrono::DateTime<Utc> {
        utc_from_local(str_to_native_datetime(local), zone)
    }

    #[test]
    fn utc_from_local_offsets() {
        let cases = [
            ("2023-06-02 00:00:00", SYDNEY, "2023-06-01 14:00:00"), // winter AEDT off
            ("2023-12-15 00:00:00", SYDNEY, "2023-12-14 13:00:00"), // summer AEDT on
            ("2023-12-15 00:00:00", BRISBANE, "2023-12-14 14:00:00"), // no DST
        ];
        for (local, zone, expected) in cases {
            assert_eq!(
                utc_local(local, zone),
                Utc.from_utc_datetime(&str_to_native_datetime(expected)),
                "{local} in {zone}"
            );
        }
    }

    #[test]
    #[should_panic(
        expected = "Ambiguous or nonexistent local time 2023-04-02 02:30:00 in timezone Australia/Sydney"
    )]
    fn test_sydney_ambiguous() {
        assert_eq!(
            utc_local("2023-04-02 02:30:00", SYDNEY),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-04-01 16:30:00"))
        );
    }

    #[test]
    #[should_panic(
        expected = "Ambiguous or nonexistent local time 2023-10-01 02:30:00 in timezone Australia/Sydney"
    )]
    fn test_sydney_nonexistent() {
        assert_eq!(
            utc_local("2023-10-01 02:30:00", SYDNEY),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-09-30 16:30:00"))
        );
    }
}
