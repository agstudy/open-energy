use crate::models::{HourMinute, InvalidHourMinute, RateBlock, RatePeriod, Tariff, TimeBand};
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;

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

pub struct TariffFactory;

impl TariffFactory {
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
    pub fn time_of_use(peak: Window, off_peak: Window, supply_rate: Decimal) -> Tariff {
        let import_tariff =
            TariffFactory::periods_for_window(off_peak.rate, off_peak.start, peak.start.prev())
                .into_iter()
                .chain(TariffFactory::periods_for_window(
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
}
