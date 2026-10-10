use bill_pricer::{
    pricer::price,
    tariff::{ConsumptionPeriod, Tariff, TariffBuilder},
};
use chrono_tz::Tz;
use criterion::{Criterion, criterion_group, criterion_main};
use domain::{HourMinute, meter::PricingInput};
use gen_meter::generator::{GeneratorConfig, generate_smart_meter};
use rust_decimal_macros::dec;
use std::hint::black_box;
use std::str::FromStr;

const SYDNEY: &str = "Australia/Sydney";

fn generate_tariff() -> Tariff {
    let hmop0000 = HourMinute::new_unchecked(0, 0);
    let hmop1759 = HourMinute::new_unchecked(17, 59);
    let hmp1800 = HourMinute::new_unchecked(18, 0);
    let hmp2059 = HourMinute::new_unchecked(20, 59);
    let hmo2100 = HourMinute::new_unchecked(21, 0);
    let hmo2359 = HourMinute::new_unchecked(23, 59);

    TariffBuilder::default()
        .daily_supply(dec!(1.0))
        .consumption_period(ConsumptionPeriod::Day)
        .rate_period(|s| {
            s.rates(&[(dec!(0.2), dec!(0)), (dec!(0.4), dec!(10))])
                .time_band(hmop0000, hmop1759, None)
        })
        .unwrap()
        .rate_period(|s| {
            s.rates(&[(dec!(0.4), dec!(0)), (dec!(0.6), dec!(10))])
                .time_band(hmp1800, hmp2059, None)
        })
        .unwrap()
        .rate_period(|s| {
            s.rates(&[(dec!(0.2), dec!(0)), (dec!(0.4), dec!(10))])
                .time_band(hmo2100, hmo2359, None)
        })
        .unwrap()
        .export_rate_period(|s| {
            s.rates(&[(dec!(0.05), dec!(0))])
                .time_band(HourMinute::min(), HourMinute::max(), None)
        })
        .unwrap()
        .build()
        .unwrap()
}

fn setup(frequency: i64) -> (Tariff, PricingInput) {
    let smart_meter = Box::leak(Box::new(
        generate_smart_meter(
            &GeneratorConfig {
                frequency,
                daily_kwh: dec!(24),
                with_export: true,
                ..Default::default()
            },
            SYDNEY.into(),
        )
        .unwrap(),
    ));

    let pricing_input = PricingInput::new(Tz::from_str(SYDNEY).unwrap(), smart_meter);
    let tariff = generate_tariff();
    (tariff, pricing_input)
}

fn criterion_benchmark(c: &mut Criterion) {
    for freq in [60, 30, 5, 1] {
        let (tariff, input) = setup(freq);
        c.bench_function(&format!("price {freq}"), |b| {
            b.iter(|| price(black_box(&tariff), black_box(&input)).unwrap())
        });
    }
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
