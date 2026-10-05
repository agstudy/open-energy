use crate::tariff::PricingError;
use crate::tariff::Tariff;

use domain::meter::PricingInput;
use rust_decimal::Decimal;

pub fn price(tariff: &Tariff, pricing_input: &PricingInput) -> Result<Decimal, PricingError> {
    if pricing_input.is_empty() {
        return Err(PricingError::NoData);
    }

    let supply_charge = tariff.supply_rate() * Decimal::from(pricing_input.days());

    let import = pricing_input
        .readings()
        .iter()
        .try_fold(Decimal::ZERO, |acc, x| {
            let v = tariff.rate_at(x.local.hour_minute, x.local.weekday)?;
            Ok::<Decimal, PricingError>(acc + x.meter.import * v)
        })?;
    Ok(import + supply_charge)
}

#[cfg(test)]
mod tests {

    use crate::{
        WeekDays,
        tariff::{TariffBuilder, TariffError},
        tariff_factory::{TariffFactory, Window},
        tests_utils::{
            GeneratorConfig, generate_smart_meter, get_tou_tariff, str_to_native_date,
            str_to_native_datetime, utc_from_local,
        },
    };

    use super::*;
    use domain::{hour_minute::HourMinute, meter::MeterMeasure};
    use rust_decimal_macros::dec;

    use chrono::{NaiveDate, TimeZone, Utc};
    use chrono_tz::Tz;
    use std::{collections::HashMap, str::FromStr};

    #[test]
    fn test_sydney_winter_midnight() {
        assert_eq!(
            utc_from_local(
                str_to_native_datetime("2023-06-02 00:00:00"),
                "Australia/Sydney"
            ),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-06-01 14:00:00"))
        );
    }
    #[test]
    fn test_sydney_summer_midnight() {
        assert_eq!(
            utc_from_local(
                str_to_native_datetime("2023-12-15 00:00:00"),
                "Australia/Sydney"
            ),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-12-14 13:00:00"))
        );
    }

    #[test]
    fn test_brisbane_summer_midnight() {
        assert_eq!(
            utc_from_local(
                str_to_native_datetime("2023-12-15 00:00:00"),
                "Australia/Brisbane"
            ),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-12-14 14:00:00"))
        );
    }

    #[test]
    #[should_panic(
        expected = "Ambiguous or nonexistent local time 2023-04-02 02:30:00 in timezone Australia/Sydney"
    )]
    fn test_sydney_ambiguous() {
        assert_eq!(
            utc_from_local(
                str_to_native_datetime("2023-04-02 02:30:00"),
                "Australia/Sydney"
            ),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-04-01 16:30:00"))
        );
    }

    #[test]
    #[should_panic]
    fn test_sydney_nonexistent() {
        assert_eq!(
            utc_from_local(
                str_to_native_datetime("2023-10-01 02:30:00"),
                "Australia/Sydney"
            ),
            Utc.from_utc_datetime(&str_to_native_datetime("2023-09-30 16:30:00"))
        );
    }
    #[test]
    fn test_time_of_use_across_midnight() {
        let time_zone = "Australia/Sydney";
        let tariff = TariffFactory::time_of_use(
            &Window {
                rate: dec!(0.4),
                start: HourMinute::new(16, 0).unwrap(),
            }, // peak: 16:00–20:59
            &Window {
                rate: dec!(0.2),
                start: HourMinute::new(21, 0).unwrap(),
            }, // off-peak: 21:00–15:59
            dec!(1.0),
        )
        .unwrap();

        let smart_meter = vec![
            MeterMeasure {
                utc_start: utc_from_local(str_to_native_datetime("2026-01-01 17:00:00"), time_zone), //peak
                import: dec!(5.0),
                export: None,
            },
            MeterMeasure {
                utc_start: utc_from_local(str_to_native_datetime("2026-01-01 23:00:00"), time_zone), //off-peak
                import: dec!(10.0),
                export: None,
            },
        ];

        // 5 * 0.4 (peak) + 10 * 0.2 (off-peak) + 1.0 (supply, 1 day) = 5.0

        let tz = Tz::from_str(time_zone).unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);

        assert_eq!(price(&tariff, &pricing_input).unwrap(), dec!(5.0));
    }

    #[test]
    fn test_sydney_brisbane_daily_light() {
        let tariff = get_tou_tariff(dec!(0.4), dec!(0.2), "16:00", "21:00");
        let timezone = "Australia/Sydney";
        let smart_meter = vec![
            MeterMeasure {
                utc_start: utc_from_local(str_to_native_datetime("2026-01-01 16:30:00"), timezone), //peak
                import: dec!(10.0),
                export: None,
            },
            MeterMeasure {
                utc_start: utc_from_local(str_to_native_datetime("2026-01-01 21:30:00"), timezone), //off-peak
                import: dec!(20.0),
                export: None,
            },
        ];
        let tz = Tz::from_str(timezone).unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        assert_eq!(price(&tariff, &pricing_input).unwrap(), dec!(9.0));

        let timezone = "Australia/Brisbane";
        let tz = Tz::from_str(timezone).unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        assert_eq!(price(&tariff, &pricing_input).unwrap(), dec!(11.0));
    }

    #[test]
    fn test_day_count() {
        let timezone = "Australia/Sydney";
        let smart_meter = generate_smart_meter(&GeneratorConfig::default());
        let tz = Tz::from_str(timezone).unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);

        assert_eq!(pricing_input.days(), 365);
        assert_eq!(pricing_input.len(), 365 * 48);
    }

    #[test]
    fn test_singular_dates_count() {
        let timezone = "Australia/Sydney";
        let smart_meter = generate_smart_meter(&GeneratorConfig::default());

        let tz = Tz::from_str(timezone).unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        let mut map: HashMap<NaiveDate, u8> = HashMap::new();
        for r in pricing_input.readings() {
            *map.entry(r.local.local_date).or_default() += 1;
        }

        let april_05 = str_to_native_date("2026-04-05");
        let oct_04 = str_to_native_date("2026-10-04");

        assert_eq!(map.get(&april_05).unwrap(), &50);
        assert_eq!(map.get(&oct_04).unwrap(), &46);
        let excluded = [april_05, oct_04];

        assert!(
            !map.iter()
                .filter(|(k, _)| !excluded.contains(*k))
                .any(|(_, v)| v != &48)
        );
    }

    #[test]
    fn test_flat() {
        let smart_meter = generate_smart_meter(&GeneratorConfig {
            end: "2026-01-02 00:00:00".into(),
            ..Default::default()
        });
        let flat_tariff = TariffFactory::flat(dec!(1), dec!(1.0)).unwrap();

        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        //24*0.2
        assert_eq!(price(&flat_tariff, &pricing_input).unwrap(), dec!(25));
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

    #[test]
    fn test_weekend_weekday_both_full_day() {
        // 1 week meter
        let smart_meter = generate_smart_meter(&GeneratorConfig {
            end: "2026-01-08 00:00:00".into(),
            ..Default::default()
        });
        let flat_rate = dec!(0.2);

        let flat_tariff = || -> Result<Tariff, TariffError> {
            TariffBuilder::default()
                .daily_supply(dec!(1.0))
                .rate_period(|s| {
                    s.rates(&[(flat_rate, dec!(0))]).time_band(
                        HourMinute::min(),
                        HourMinute::max(),
                        Some(WeekDays::working_days()),
                    )
                })?
                .rate_period(|s| {
                    s.rates(&[(dec!(0.5), dec!(0))]).time_band(
                        HourMinute::min(),
                        HourMinute::max(),
                        Some(WeekDays::weekend()),
                    )
                })?
                .build()
        };
        assert!(matches!(&flat_tariff(), Ok(_)));

        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);
        // (0.2*5+0.5*2)*24 +7*1 = 2*24+7 = 55
        assert_eq!(
            price(&flat_tariff().unwrap(), &pricing_input).unwrap(),
            dec!(55)
        );
    }

    #[test]
    fn test_only_weekday_full_day() {
        let flat_rate = dec!(0.2);

        let flat_tariff = || -> Result<Tariff, TariffError> {
            TariffBuilder::default()
                .daily_supply(dec!(1.0))
                .rate_period(|s| {
                    s.rates(&[(flat_rate, dec!(0))]).time_band(
                        HourMinute::min(),
                        HourMinute::max(),
                        Some(WeekDays::working_days()),
                    )
                })?
                .build()
        };

        assert!(matches!(
            &flat_tariff(),
            Err(TariffError::Gap(chrono::Weekday::Sat, _))
        ));
    }
    #[test]
    fn test_alldays_weekend_band() {
        let flat_rate = dec!(0.2);

        let flat_tariff = || -> Result<Tariff, TariffError> {
            TariffBuilder::default()
                .daily_supply(dec!(1.0))
                .rate_period(|s| {
                    s.rates(&[(flat_rate, dec!(0))]).time_band(
                        HourMinute::min(),
                        HourMinute::max(),
                        None,
                    )
                })?
                .rate_period(|s| {
                    s.rates(&[(flat_rate, dec!(0))]).time_band(
                        HourMinute::from_str("16:00").unwrap(),
                        HourMinute::from_str("23:59").unwrap(),
                        Some(WeekDays::weekend()),
                    )
                })?
                .build()
        };

        assert!(matches!(&flat_tariff(), Err(TariffError::Overlap(_, _))));
    }

    #[test]
    fn test_multi_days() {
        let smart_meter = generate_smart_meter(&GeneratorConfig {
            //end: "2026-01-08 00:00:00".into(),
            ..Default::default()
        });

        let tariff = TariffFactory::flat(dec!(0.0), dec!(1)).unwrap();

        let tz = Tz::from_str("Australia/Sydney").unwrap();
        let pricing_input = PricingInput::new(tz, &smart_meter);

        assert_eq!(price(&tariff, &pricing_input).unwrap(), dec!(365));
    }
}
