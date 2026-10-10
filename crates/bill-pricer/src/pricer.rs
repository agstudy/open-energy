use crate::tariff::ConsumptionPeriod;
use crate::tariff::PricingError;
use crate::tariff::Tariff;
use crate::tariff::TariffDirection;

use chrono::Datelike;
use chrono::NaiveDate;
use domain::meter::PricingInput;
use domain::meter::Reading;
use rust_decimal::Decimal;
use std::fmt;

struct Acc {
    import_price: Decimal,
    import_kwh: Decimal,
    export_price: Decimal,
    export_kwh: Decimal,
    prev_date: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct PricingResult {
    pub import: Decimal,
    pub import_kwh: Decimal,
    pub export: Decimal,
    pub export_kwh: Decimal,
    pub fixed_bill: Decimal,
    pub variable_bill: Decimal,
    pub total_bill: Decimal,
}

impl std::fmt::Display for PricingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Pricing Result")?;
        writeln!(
            f,
            "  Import:  {:>12} ({:>10} kWh)",
            self.import, self.import_kwh
        )?;
        writeln!(
            f,
            "  Export:  {:>12} ({:>10} kWh)",
            self.export, self.export_kwh
        )?;
        writeln!(f, "  Fixed:   {:>12}", self.fixed_bill)?;
        writeln!(f, "  Variable:{:>12}", self.variable_bill)?;
        write!(f, "  Total:   {:>12}", self.total_bill)
    }
}

fn reset_acc(r: &Reading, tariff: &Tariff, acc: &mut Acc) {
    if let Some(period) = tariff.consumption_period() {
        let reset = match period {
            ConsumptionPeriod::Day => acc.prev_date != Some(r.local.date),
            ConsumptionPeriod::Month => acc.prev_date.is_none_or(|d| {
                d.year() != r.local.date.year() || d.month() != r.local.date.month()
            }),
        };
        if reset {
            acc.import_kwh = Decimal::ZERO;
            acc.export_kwh = Decimal::ZERO;
        }
        acc.prev_date = Some(r.local.date);
    }
}

fn price_variable(tariff: &Tariff, pricing_input: &PricingInput) -> Result<Acc, PricingError> {
    let has_export = !tariff.export_tariff().is_empty();
    let mut acc = Acc {
        import_price: Decimal::ZERO,
        import_kwh: Decimal::ZERO,
        prev_date: None,
        export_kwh: Decimal::ZERO,
        export_price: Decimal::ZERO,
    };
    for r in pricing_input.readings() {
        reset_acc(r, tariff, &mut acc);
        // import
        let import_cost = tariff.cost_at(
            TariffDirection::Import,
            r.local.hour_minute,
            r.local.weekday,
            acc.import_kwh,
            r.meter.import,
        )?;
        acc.import_price += import_cost;
        acc.import_kwh += r.meter.import;

        // export
        if has_export && let Some(export) = r.meter.export {
            let export_cost = tariff.cost_at(
                TariffDirection::Export,
                r.local.hour_minute,
                r.local.weekday,
                acc.export_kwh,
                export,
            )?;
            acc.export_price += export_cost;
            acc.export_kwh += export;
        }
    }

    Ok(acc)
}
/// Prices the given input against a tariff.
///
/// # Errors
///
/// Returns [`PricingError`] if the tariff does not fully cover the
/// pricing input's time range, or if any day is not covered by
/// non-overlapping bands.
pub fn price(tariff: &Tariff, pricing_input: &PricingInput) -> Result<PricingResult, PricingError> {
    if pricing_input.is_empty() {
        return Err(PricingError::NoData);
    }

    let supply_charge = tariff.supply_rate() * Decimal::from(pricing_input.days());
    let result = price_variable(tariff, pricing_input)?;
    let fixed = supply_charge.round_dp(2);
    let variable = (result.import_price - result.export_price).round_dp(2);
    let total = fixed + variable;

    Ok(PricingResult {
        fixed_bill: fixed,
        export: result.export_price.round_dp(2),
        import_kwh: result.import_kwh.round_dp(2),
        import: result.import_price.round_dp(2),
        variable_bill: variable,
        export_kwh: result.export_kwh.round_dp(2),
        total_bill: total,
    })
}

#[cfg(test)]
mod tests {

    use crate::{
        WeekDays,
        tariff::{TariffBuilder, TariffError},
        tariff_factory::{TariffFactory, Window, WindowTiered},
        tests_utils::{get_tou_tariff, str_to_native_date, str_to_native_datetime, utc_from_local},
    };

    use super::*;
    use domain::{hour_minute::HourMinute, meter::MeterMeasure};
    use gen_meter::generator::{GeneratorConfig, generate_smart_meter};
    use rust_decimal_macros::dec;

    use chrono::{NaiveDate, Utc};
    use chrono_tz::Tz;
    use std::{collections::HashMap, str::FromStr};

    const SYDNEY: &str = "Australia/Sydney";
    const BRISBANE: &str = "Australia/Brisbane";

    fn tz(name: &str) -> Tz {
        Tz::from_str(name).unwrap()
    }

    fn sm(cfg: &GeneratorConfig) -> Vec<MeterMeasure> {
        generate_smart_meter(cfg, SYDNEY.into()).unwrap()
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
        let mut b = TariffBuilder::default().daily_supply(supply);
        b = b.rate_period(|s| {
            s.rates(&[(weekday, dec!(0))]).time_band(
                HourMinute::min(),
                HourMinute::max(),
                Some(WeekDays::working_days()),
            )
        })?;
        if let Some(w) = weekend {
            b = b.rate_period(|s| {
                s.rates(&[(w, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    Some(WeekDays::weekend()),
                )
            })?;
        }
        b.build()
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
        let tariff = get_tou_tariff(dec!(0.4), dec!(0.2), "16:00", "21:00");
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
        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                //end: "2026-01-08 00:00:00".into(),
                ..Default::default()
            },
            SYDNEY.into(),
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
        let tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(12))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .consumption_period(ConsumptionPeriod::Day)
            .build()
            .unwrap();

        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                start: "2026-01-01 00:00:00".into(),
                end: "2026-01-02 00:00:00".into(),
                frequency: 60,
                daily_kwh: dec!(24),
                ..Default::default()
            },
            SYDNEY.into(),
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
        let tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(0.4), dec!(200))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .consumption_period(ConsumptionPeriod::Month)
            .build()
            .unwrap();

        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                start: "2026-01-01 00:00:00".into(),
                end: "2026-04-01 00:00:00".into(),
                frequency: 60,
                daily_kwh: dec!(24),
                ..Default::default()
            },
            SYDNEY.into(),
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
            SYDNEY.into(),
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

        let multiflat_tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .consumption_period(ConsumptionPeriod::Day)
            .build()
            .unwrap();

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

        let multiflat_tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build()
            .unwrap();

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

        let multiflat_tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .consumption_period(ConsumptionPeriod::Day)
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build()
            .unwrap();

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

        let multiflat_tariff = TariffBuilder::default()
            .daily_supply(dec!(1.0))
            .rate_period(|s| {
                s.rates(&[(dec!(0.2), dec!(0)), (dec!(1), dec!(10))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build()
            .unwrap();

        // 10 *0.2 + 14x1 + 1 + 24x1 +1 =
        assert_eq!(
            price(&multiflat_tariff, &pricing_input).unwrap().total_bill,
            dec!(42)
        );
    }
}
