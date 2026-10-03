use crate::tariff::PricingError;
use crate::tariff::Tariff;

use domain::meter::PricingInput;
use rust_decimal::Decimal;

pub fn price(tariff: &Tariff, pricing_input: &PricingInput) -> Result<Decimal, PricingError> {
    if pricing_input.is_empty() {
        return Err(PricingError::NoData);
    };

    let supply_charge = tariff.supply_rate() * Decimal::from(pricing_input.days());

    let import = pricing_input
        .readings()
        .iter()
        .try_fold(Decimal::ZERO, |acc, x| {
            let v = tariff.rate_at(x.local.hour_minute, x.local.weekday)?;
            Ok::<Decimal, PricingError>(acc + x.meter.import * v)
        })?;
    // dbg!("import {}", import);
    Ok(import + supply_charge)
}

#[cfg(test)]
mod tests {

    use crate::tariff_factory::{TariffFactory, Window};

    use super::*;
    use domain::{hour_minute::HourMinute, meter::MeterMeasure};
    use rust_decimal_macros::dec;

    use chrono::{TimeZone, Utc};
    use chrono_tz::Tz;
    use std::str::FromStr;

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
        )
        .unwrap();

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

        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);

        assert_eq!(price(&tariff, &pricing_input).unwrap(), dec!(5.0));
    }

    #[test]
    fn test_flat() {
        let smart_meter = vec![MeterMeasure {
            utc_start: Utc::now(),
            import: dec!(10.0),
            export: None,
        }];

        let flat_tariff = TariffFactory::flat(dec!(0.2), dec!(1.0)).unwrap();

        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        assert_eq!(price(&flat_tariff, &pricing_input).unwrap(), dec!(3));
    }

    #[test]
    fn test_empty_smart_meter() {
        let tariff = TariffFactory::flat(dec!(0.2), dec!(1.0)).unwrap();

        let smart_meter: Vec<MeterMeasure> = vec![];
        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);

        assert!(matches!(
            price(&tariff, &pricing_input),
            Err(PricingError::NoData)
        ));
    }
}
