#[cfg(test)]
mod tests {

    use crate::{
        pricer::price,
        tariff::{ConsumptionPeriod, PricingError, Tariff},
    };

    use chrono::NaiveDate;
    use domain::meter::PricingInput;
    use rust_decimal::Decimal;

    use crate::{
        TimeBand, WeekDays,
        tariff::{TariffBuilder, TariffError},
        tariff::{TariffFactory, Window, WindowTiered},
        tests_utils::{get_tou_tariff, str_to_native_date, str_to_native_datetime, utc_from_local},
    };

    use domain::{hour_minute::HourMinute, meter::MeterMeasure, year_month::SeasonRange};
    use gen_meter::generator::{GeneratorConfig, generate_smart_meter};
    use rust_decimal_macros::dec;

    use chrono::Utc;
    use chrono_tz::Tz;
    use std::{collections::HashMap, str::FromStr};

    const SYDNEY: &str = "Australia/Sydney";
    const BRISBANE: &str = "Australia/Brisbane";

    fn tz(name: &str) -> Tz {
        Tz::from_str(name).unwrap()
    }

    fn sm(cfg: &GeneratorConfig) -> Vec<MeterMeasure> {
        generate_smart_meter(cfg, SYDNEY).unwrap()
    }

    fn priced(meter: &[MeterMeasure], zone: &str) -> PricingInput {
        PricingInput::new(tz(zone), meter)
    }

    fn utc_local(local: &str, zone: &str) -> chrono::DateTime<Utc> {
        utc_from_local(str_to_native_datetime(local), zone)
    }

    fn flat_tariff(
        weekday: Decimal,
        weekend: Option<Decimal>,
        supply: Decimal,
    ) -> Result<Tariff, TariffError> {
        let mut builder = TariffBuilder::default();
        builder.daily_supply(supply);
        builder.rate_period(SeasonRange::full_year(), |s| {
            s.rates(&[(weekday, dec!(0))]).time_band(
                HourMinute::min(),
                HourMinute::max(),
                Some(WeekDays::working_days()),
            )
        })?;
        if let Some(w) = weekend {
            builder.rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(w, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    Some(WeekDays::weekend()),
                )
            })?;
        }
        builder.build()
    }

    #[test]
    fn test_time_of_use_across_midnight() {
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
                utc_start: utc_local("2026-01-01 17:00:00", SYDNEY),
                import: dec!(5.0),
                export: None,
            },
            MeterMeasure {
                utc_start: utc_local("2026-01-01 23:00:00", SYDNEY),
                import: dec!(10.0),
                export: None,
            },
        ];

        // 5 * 0.4 (peak) + 10 * 0.2 (off-peak) + 1.0 (supply, 1 day) = 5.0

        let pricing_input = priced(&smart_meter, SYDNEY);
        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(5.0)
        );
    }

    #[test]
    fn test_sydney_brisbane_daily_light() {
        let tariff = get_tou_tariff(dec!(0.4), dec!(0.2), "16:00", "21:00").unwrap();
        let smart_meter = vec![
            MeterMeasure {
                utc_start: utc_local("2026-01-01 16:30:00", SYDNEY), //peak
                import: dec!(10.0),
                export: None,
            },
            MeterMeasure {
                utc_start: utc_local("2026-01-01 21:30:00", SYDNEY), //off-peak
                import: dec!(20.0),
                export: None,
            },
        ];
        let pricing_input = priced(&smart_meter, SYDNEY);
        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(9.0)
        );
        let pricing_input = priced(&smart_meter, BRISBANE);
        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(11.0)
        );
    }

    #[test]
    fn test_day_count() {
        let smart_meter = sm(&GeneratorConfig::default());
        let pricing_input = priced(&smart_meter, SYDNEY);

        assert_eq!(pricing_input.days(), 365);
        assert_eq!(pricing_input.len(), 365 * 48);
    }

    #[test]
    fn test_singular_dates_count() {
        let smart_meter = sm(&GeneratorConfig::default());

        let pricing_input = priced(&smart_meter, SYDNEY);

        let mut map: HashMap<NaiveDate, u8> = HashMap::new();
        for r in pricing_input.readings() {
            *map.entry(r.local.date).or_default() += 1;
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
        let smart_meter = sm(&GeneratorConfig {
            end: "2026-01-02 00:00:00".into(),
            ..Default::default()
        });
        let flat_tariff = TariffFactory::flat(dec!(1), dec!(1.0)).unwrap();

        let pricing_input = priced(&smart_meter, SYDNEY);

        //24*1 +1 = 25
        assert_eq!(
            price(&flat_tariff, &pricing_input).unwrap().total_bill,
            dec!(25)
        );
    }

    #[test]
    fn test_empty_smart_meter() {
        let tariff = TariffFactory::flat(dec!(0.2), dec!(1.0)).unwrap();

        let smart_meter: Vec<MeterMeasure> = vec![];
        let pricing_input = priced(&smart_meter, SYDNEY);

        assert!(matches!(
            price(&tariff, &pricing_input),
            Err(PricingError::NoData)
        ));
    }

    #[test]
    fn test_weekend_weekday_both_full_day() {
        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                end: "2026-01-08 00:00:00".into(),
                ..Default::default()
            },
            SYDNEY,
        )
        .unwrap();
        let tariff = flat_tariff(dec!(0.2), Some(dec!(0.5)), dec!(1.0)).unwrap();
        let input = priced(&smart_meter, SYDNEY);
        // (0.2*5 + 0.5*2)*24 + 7*1 = 55
        assert_eq!(price(&tariff, &input).unwrap().total_bill, dec!(55));
    }

    #[test]
    fn test_only_weekday_full_day() {
        let tariff = flat_tariff(dec!(0.2), None, dec!(1.0));
        assert!(matches!(
            &tariff,
            Err(TariffError::Gap(chrono::Weekday::Sat, _))
        ));
    }

    #[test]
    fn test_alldays_weekend_band() {
        let flat_rate = dec!(0.2);

        let flat_tariff = || -> Result<Tariff, TariffError> {
            let mut builder = TariffBuilder::default();
            builder.daily_supply(dec!(1.0));

            builder.rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(flat_rate, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })?;
            builder.rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(flat_rate, dec!(0))]).time_band(
                    HourMinute::from_str("16:00").unwrap(),
                    HourMinute::from_str("23:59").unwrap(),
                    Some(WeekDays::weekend()),
                )
            })?;
            builder.build()
        };

        assert!(matches!(&flat_tariff(), Err(TariffError::Overlap(_, _))));
    }

    #[test]
    fn test_multi_days() {
        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                //end: "2026-01-08 00:00:00".into(),
                ..Default::default()
            },
            SYDNEY,
        )
        .unwrap();

        let tariff = TariffFactory::flat(dec!(0.0), dec!(1)).unwrap();

        let pricing_input = priced(&smart_meter, SYDNEY);

        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(365)
        );
    }

    #[test]
    fn test_multiflat_daily() {
        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(1.0));
        builder.consumption_period(Some(ConsumptionPeriod::Day));
        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(12))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                start: "2026-01-01 00:00:00".into(),
                end: "2026-01-02 00:00:00".into(),
                frequency: 60,
                daily_kwh: dec!(24),
                ..Default::default()
            },
            SYDNEY,
        )
        .unwrap();
        // 12*0.2 +12*1 + 1 = 15.4
        let pricing_input = priced(&smart_meter, SYDNEY);

        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(15.4)
        );
    }

    #[test]
    fn test_multiflat_monthly() {
        let mut builder = TariffBuilder::default();
        builder.consumption_period(Some(ConsumptionPeriod::Month));
        builder.daily_supply(dec!(1.0));

        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(0.4), dec!(200))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                start: "2026-01-01 00:00:00".into(),
                end: "2026-04-01 00:00:00".into(),
                frequency: 60,
                daily_kwh: dec!(24),
                ..Default::default()
            },
            SYDNEY,
        )
        .unwrap();
        // Jan (31d): 200*0.2 + (744-200)*0.4 = 257.6
        // Feb (28d): 200*0.2 + (672-200)*0.4 = 228.8
        // Mar (31d): same as Jan = 257.6
        // Variable = 744, Supply = 90 days * 1.0 = 90, Total = 834
        let pricing_input = priced(&smart_meter, SYDNEY);

        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(834)
        );
    }

    #[test]
    fn price_export() {
        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                end: "2027-01-01 00:00:00".into(),
                with_export: true,
                system_capacity: 1,
                daily_kwh: dec!(0),
                ..Default::default()
            },
            SYDNEY,
        )
        .unwrap();
        let flat_tariff = TariffFactory::flat_export(dec!(1), dec!(0.0), dec!(1)).unwrap();

        let pricing_input = priced(&smart_meter, SYDNEY);

        assert_eq!(
            price(&flat_tariff, &pricing_input)
                .unwrap()
                .total_bill
                .round(),
            dec!(-1493)
        );
    }

    #[test]
    fn test_multiflat_split_reading_at_threshold() {
        let smart_meter = sm(&GeneratorConfig {
            end: "2026-01-02 00:00:00".into(),
            daily_kwh: dec!(36),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        let mut builder = TariffBuilder::default();
        builder.consumption_period(Some(ConsumptionPeriod::Day));
        builder.daily_supply(dec!(1.0));

        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let multiflat_tariff = builder.build().unwrap();

        // 10 *0.2 + 1* (36-10) +1 = 2+ 26 +1 = 29
        assert_eq!(
            price(&multiflat_tariff, &pricing_input).unwrap().total_bill,
            dec!(29)
        );
    }

    #[test]
    fn test_tiered_priscing_with_no_consumption_period() {
        let smart_meter = sm(&GeneratorConfig {
            end: "2026-01-02 00:00:00".into(),
            daily_kwh: dec!(36),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(1.0));
        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let multiflat_tariff = builder.build().unwrap();

        // 10 *0.2 + 1* (36-10) +1 = 2+ 26 +1 = 29
        assert_eq!(
            price(&multiflat_tariff, &pricing_input).unwrap().total_bill,
            dec!(29)
        );
    }

    #[test]
    fn shared_counter_crosses_tier_across_bands() {
        let smart_meter = sm(&GeneratorConfig {
            end: "2026-01-02 00:00:00".into(),
            daily_kwh: dec!(12),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        let tariff = TariffFactory::time_of_use_multiflat(
            &WindowTiered {
                rates: vec![(dec!(0.5), dec!(0)), (dec!(0.8), dec!(10))],
                start: HourMinute::new(16, 0).unwrap(),
            }, // peak: 16:00–23:59
            &WindowTiered {
                rates: vec![(dec!(0.2), dec!(0)), (dec!(0.3), dec!(10))],
                start: HourMinute::new(0, 0).unwrap(),
            }, // off-peak: 00:00–15:59
            dec!(1.0),
        )
        .unwrap();

        // 8 *0.2(<16h) + 2*0.5(16h-20h)+2*0.8(20h-24h)+1 =1.6+1+1.6+1=5.2
        assert_eq!(
            price(&tariff, &pricing_input).unwrap().total_bill,
            dec!(5.2)
        );
    }

    #[test]
    fn test_good_reset_tiered_daylight_saving() {
        let smart_meter = sm(&GeneratorConfig {
            start: "2026-04-04 00:00:00".into(),
            end: "2026-04-07 00:00:00".into(),
            daily_kwh: dec!(24),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        let mut builder = TariffBuilder::default();
        builder.consumption_period(Some(ConsumptionPeriod::Day));
        builder.daily_supply(dec!(1.0));

        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let multiflat_tariff = builder.build().unwrap();
        // 10 *0.2 + 14x1 +1 +10x0.2+15*1+1+10 *0.2 + 14x1 +1= 17+ 18+ 17 = 52
        assert_eq!(
            price(&multiflat_tariff, &pricing_input).unwrap().total_bill,
            dec!(52)
        );
    }

    #[test]
    fn test_good_no_daily_reset_multiflat() {
        let smart_meter = sm(&GeneratorConfig {
            start: "2026-01-01 00:00:00".into(),
            end: "2026-01-03 00:00:00".into(),
            daily_kwh: dec!(24),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(1.0));
        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let multiflat_tariff = builder.build().unwrap();

        // 10 *0.2 + 14x1 + 1 + 24x1 +1 =
        assert_eq!(
            price(&multiflat_tariff, &pricing_input).unwrap().total_bill,
            dec!(42)
        );
    }

    #[test]
    fn test_two_seasons() {
        // summer: Apr-Sep, one period; winter: Oct-Mar, another period
        // build().unwrap()
        // cost_at in summer vs winter returns different rates

        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(0.0));

        builder.add_rate_period(
            SeasonRange {
                start_date: "04-01".try_into().unwrap(), // MM-DD
                end_date: "09-30".try_into().unwrap(),   // MM-DD
            },
            &[(dec!(1) / dec!(24), Decimal::ZERO)],
            TimeBand::full_day(),
        );
        builder.add_rate_period(
            SeasonRange {
                start_date: "10-01".try_into().unwrap(), // MM-DD
                end_date: "03-31".try_into().unwrap(),   // MM-DD
            },
            &[(dec!(2) / dec!(24), Decimal::ZERO)],
            TimeBand::full_day(),
        );

        let summer_winter = builder.build().unwrap();

        let smart_meter = sm(&GeneratorConfig {
            daily_kwh: dec!(24),
            start: "2026-09-01 00:00:00".into(),
            end: "2026-10-31 00:00:00".into(),
            ..Default::default()
        });
        let pricing_input = priced(&smart_meter, SYDNEY);

        // (summer_rate * 30 + winter_rate *30)*24 = 30 *(w+s)/24 *24 = 90
        // end is exclusive
        assert_eq!(
            price(&summer_winter, &pricing_input)
                .unwrap()
                .total_bill
                .round(),
            dec!(90)
        );
    }
}
