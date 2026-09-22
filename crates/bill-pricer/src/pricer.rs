use std::cmp::max;

use crate::models::{HourMinute, InvalidHourMinute, RateBlock, RatePeriod, Tariff, TimeBand};
use chrono::{DateTime, Timelike, Utc};
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
    NoRates,
    NoTariff,
    InvalidHourMinute(InvalidHourMinute),
    CorruptedData, // TODO: not yet used
}

pub struct Window {
    pub rate: Decimal,
    pub start: HourMinute,
}

impl Tariff {
    pub fn flat(flat_rate: Decimal, supply_rate: Decimal) -> Tariff {
        Tariff {
            import_tariff: vec![RatePeriod {
                rates: vec![RateBlock {
                    rate: flat_rate,
                    lower_band: None,
                }],
                time_band: None,
            }],
            export_tariff: None,
            discount: None,
            supply_rate,
        }
    }

    fn periods_for_window(rate: Decimal, start: HourMinute, end: HourMinute) -> Vec<RatePeriod> {
        start
            .split_midnight(end)
            .into_iter()
            .map(|(s, e)| {
                RatePeriod::new(
                    rate,
                    Some(TimeBand {
                        start: s,
                        end: e,
                        days_of_week: None,
                    }),
                )
            })
            .collect()
    }

    pub fn time_of_use(peak: Window, off_peak: Window, supply_rate: Decimal) -> Tariff {
        let import_tariff =
            Tariff::periods_for_window(off_peak.rate, off_peak.start, peak.start.prev())
                .into_iter()
                .chain(Tariff::periods_for_window(
                    peak.rate,
                    peak.start,
                    off_peak.start.prev(),
                ))
                .collect();

        Tariff {
            import_tariff,
            export_tariff: None,
            discount: None,
            supply_rate,
        }
    }

    pub fn rate_at(&self, at: DateTime<Utc>) -> Result<Decimal, PricingError> {
        let at_hm =
            HourMinute::new(at.hour(), at.minute()).map_err(PricingError::InvalidHourMinute)?;

        let matches: Vec<&RatePeriod> = self
            .import_tariff
            .iter()
            .filter(|p| p.applies_at(&at_hm))
            .collect();

        match matches.as_slice() {
            [] => Err(PricingError::NoTariff),
            [period] => period
                .rates
                .first()
                .ok_or(PricingError::NoRates)
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

    use crate::models::RatePeriod;

    use super::*;
    use rust_decimal_macros::dec;

    use chrono::TimeZone;

    #[test]
    fn test_time_of_use_across_midnight() {
        let tariff = Tariff::time_of_use(
            Window {
                rate: dec!(0.4),
                start: HourMinute::new(16, 0).unwrap(),
            }, // peak: 16:00–20:59
            Window {
                rate: dec!(0.2),
                start: HourMinute::new(21, 0).unwrap(),
            }, // off-peak: 21:00–15:59
            dec!(1.0),
        );

        let smart_meter = vec![
            MeterMeasure {
                utc_start: Utc.with_ymd_and_hms(2026, 1, 1, 17, 0, 0).unwrap(), // peak
                import: dec!(10.0),
                export: None,
            },
            MeterMeasure {
                utc_start: Utc.with_ymd_and_hms(2026, 1, 1, 23, 0, 0).unwrap(), // off-peak
                import: dec!(10.0),
                export: None,
            },
        ];

        // 10 * 0.4 (peak) + 10 * 0.2 (off-peak) + 1.0 (supply, 1 day) = 7.0
        assert_eq!(price(&tariff, &smart_meter).unwrap(), dec!(7.0));
    }

    #[test]
    fn test_flat() {
        let smart_meter = vec![MeterMeasure {
            utc_start: Utc::now(),
            import: dec!(10.0),
            export: None,
        }];

        let flat_tariff = Tariff::flat(dec!(0.2), dec!(1.0));

        assert_eq!(price(&flat_tariff, &smart_meter).unwrap(), dec!(3));
    }

    #[test]
    fn test_empty_rates_on_matched_period() {
        let tariff = Tariff {
            import_tariff: vec![RatePeriod {
                rates: vec![],
                time_band: None,
            }],
            export_tariff: None,
            discount: None,
            supply_rate: dec!(1.0),
        };
        let smart_meter = vec![MeterMeasure {
            utc_start: Utc::now(),
            import: dec!(10.0),
            export: None,
        }];

        assert!(matches!(
            price(&tariff, &smart_meter),
            Err(PricingError::NoRates)
        ));
    }

    #[test]
    fn test_empty_tariff() {
        let smart_meter = vec![MeterMeasure {
            utc_start: Utc::now(),
            import: dec!(10.0),
            export: None,
        }];

        let tariff = Tariff {
            import_tariff: vec![],
            export_tariff: None,
            discount: None,
            supply_rate: dec!(1.0),
        };

        assert!(matches!(
            price(&tariff, &smart_meter),
            Err(PricingError::NoTariff)
        ));
    }

    #[test]
    fn test_empty_smart_meter() {
        let tariff = Tariff::flat(dec!(0.2), dec!(1.0));

        let smart_meter: Vec<MeterMeasure> = vec![];

        assert!(matches!(
            price(&tariff, &smart_meter),
            Err(PricingError::NoData)
        ));
    }
}
