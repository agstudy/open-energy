use std::cmp::max;

use crate::models::{HourMinute, RatePeriod, Tariff};
use chrono::{DateTime, Datelike, Timelike, Utc};
use rust_decimal::Decimal;

pub struct MeterMeasure {
    pub utc_start: DateTime<Utc>,
    pub import: Decimal,
    pub export: Option<Decimal>,
}

#[derive(Debug)]
pub enum PricingError {
    TariffOverlapp,
    NoData,
    CorruptedData,
}

impl Tariff {
    pub fn rate_at(&self, at: DateTime<Utc>) -> Result<Decimal, PricingError> {
        let at_hm = HourMinute {
            minute: at.minute(),
            hour: at.hour(),
        };

        let week_day = at.weekday();

        let matches: Vec<&RatePeriod> = self
            .import_tariff
            .iter()
            .filter(|p| p.applies_at(&at_hm))
            .collect();

        match matches.as_slice() {
            [] => Err(PricingError::NoData),
            [period] => period
                .rates
                .first()
                .ok_or(PricingError::NoData)
                .map(|b| b.rate),
            _ => Err(PricingError::TariffOverlapp),
        }
    }
}
pub fn price(tariff: &Tariff, smart_meter: &[MeterMeasure]) -> Result<Decimal, PricingError> {
    if smart_meter.is_empty() {
        return Err(PricingError::NoData);
    };

    let days = if let (Some(first), Some(last)) = (smart_meter.first(), smart_meter.last()) {
        max((last.utc_start - first.utc_start).num_days(), 1)
    } else {
        0
    };

    let supply_charge: Decimal = tariff.supply_rate * Decimal::from(days);

    let import = smart_meter.iter().try_fold(Decimal::ZERO, |acc, x| {
        let v = tariff.rate_at(x.utc_start)?;
        Ok(acc + x.import * v)
    })?;
    Ok(import + supply_charge)
}

#[cfg(test)]
mod tests {

    use crate::models::{RateBlock, RatePeriod};

    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_flat() {
        let smart_meter = vec![MeterMeasure {
            utc_start: Utc::now(),
            import: dec!(10.0),
            export: None,
        }];

        let flat_tariff = Tariff {
            import_tariff: vec![RatePeriod {
                rates: vec![RateBlock {
                    rate: dec!(0.2),
                    lower_band: None,
                }],
                time_band: None,
            }],
            export_tariff: None,
            discount: None,
            supply_rate: dec!(1.0),
        };

        assert_eq!(price(&flat_tariff, &smart_meter).unwrap(), dec!(3));
    }
}
