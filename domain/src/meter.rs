use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

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
