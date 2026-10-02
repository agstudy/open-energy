use chrono::{Datelike, NaiveDate, Timelike, Weekday};
use chrono_tz::Tz;
use std::{
    collections::{BTreeMap, HashSet}
};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::hour_minute::HourMinute;
#[derive(Debug, Clone)]
pub struct MeterMeasure {
    pub utc_start: DateTime<Utc>,
    pub import: Decimal,
    pub export: Option<Decimal>,
}

impl MeterMeasure {
    pub fn new(utc_start: DateTime<Utc>, import: Decimal) -> Self {
        MeterMeasure {
            utc_start,
            import,
            export: None,
        }
    }

    pub fn to_local(&self, tz: Tz) -> LocalMeasure {
        let local = self.utc_start.with_timezone(&tz);
        LocalMeasure {
            local_date: local.date_naive(),
            hour_minute: HourMinute::new_unchecked(local.hour(), local.minute()),
            weekday: local.weekday(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalMeasure {
    pub local_date: NaiveDate,
    pub hour_minute: HourMinute,
    pub weekday: Weekday,
}

#[derive(Debug, Clone)]
pub struct Reading {
    pub local: LocalMeasure,
    pub meter: MeterMeasure,
}

#[derive(Debug, Clone)]
pub struct PricingInput {
    timezone: Tz,
    measures: Vec<Reading>,
}

fn create_readings(tz: Tz, values: &[MeterMeasure]) -> Vec<Reading> {
    values
        .iter()
        .map(|meter| {
            let local = meter.to_local(tz);
            Reading {
                local,
                meter: meter.clone(),
            }
        })
        .collect()
}

impl PricingInput {
    pub fn new(tz: Tz, values: &[MeterMeasure]) -> Self {
        Self {
            timezone: tz,
            measures: create_readings(tz, values),
        }
    }

    pub fn readings(&self) -> &[Reading] {
        self.measures.as_slice()
    }

    pub fn timezone(&self) -> Tz {
        self.timezone
    }

    pub fn is_empty(&self) -> bool {
        self.measures.is_empty()
    }

    pub fn days(&self) -> usize {
        self.measures
            .iter()
            .map(|v| v.local.local_date)
            .collect::<HashSet<NaiveDate>>()
            .len()
    }
}

/// Merges import and export series by timestamp. Export-only intervals are
/// treated as zero-import (e.g. solar export with no simultaneous grid draw).
/// Import-only intervals (no export reading) are the common case pre-solar-install
/// and are left with `export: None`.
pub fn merge_import_export(
    import: &[(DateTime<Utc>, Decimal)],
    export: &[(DateTime<Utc>, Decimal)],
) -> Vec<MeterMeasure> {
    let mut import_tree: BTreeMap<DateTime<Utc>, MeterMeasure> = import
        .iter()
        .map(|&(ts, val)| (ts, MeterMeasure::new(ts, val)))
        .collect();

    for &(key, value) in export {
        import_tree
            .entry(key)
            .or_insert_with(|| MeterMeasure::new(key, Decimal::ZERO))
            .export = Some(value);
    }

    import_tree.into_values().collect()
}
