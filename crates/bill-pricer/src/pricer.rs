use std::{cmp::max};

use crate::{models::Tariff, tariff::PricingError};
use domain::meter::MeterMeasure;
use rust_decimal::Decimal;



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

    use crate::{
        models::{HourMinute, RatePeriod},
        tariff::{TariffFactory, Window},
    };

    use super::*;
    use rust_decimal_macros::dec;

    use chrono::{TimeZone, Utc};

    #[test]
    fn test_time_of_use_across_midnight() {
        let tariff = TariffFactory::time_of_use(
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

        let flat_tariff = TariffFactory::flat(dec!(0.2), dec!(1.0));

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
        let tariff = TariffFactory::flat(dec!(0.2), dec!(1.0));

        let smart_meter: Vec<MeterMeasure> = vec![];

        assert!(matches!(
            price(&tariff, &smart_meter),
            Err(PricingError::NoData)
        ));
    }
}
