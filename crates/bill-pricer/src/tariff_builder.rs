use rust_decimal::Decimal;

use crate::models::{HourMinute, TimeBand, WeekDays};

use crate::tariff::{Discount, RateBlock, RatePeriod, Tariff, TariffError};

pub struct RatePeriodBuilder {
    rates: Vec<RateBlock>,
    time_band: TimeBand,
}

impl Default for RatePeriodBuilder {
    fn default() -> Self {
        Self {
            rates: Default::default(),
            time_band: TimeBand::full_day(),
        }
    }
}

impl RatePeriodBuilder {
    pub fn rates(mut self, rates: &[(Decimal, Decimal)]) -> Self {
        self.rates = rates
            .iter()
            .map(|(r, b)| RateBlock {
                rate: *r,
                lower_band: *b,
            })
            .collect();
        self
    }

    pub fn time_band(
        mut self,
        start: HourMinute,
        end: HourMinute,
        days_of_week: Option<WeekDays>,
    ) -> Result<Self, TariffError> {
        self.time_band = TimeBand::new(start, end, days_of_week)?;
        Ok(self)
    }

    pub fn build(self) -> RatePeriod {
        RatePeriod {
            rates: self.rates,
            time_band: self.time_band,
        }
    }
}

#[derive(Default)]
pub struct TariffBuilder {
    import_tariff: Vec<RatePeriod>,
    export_tariff: Option<Vec<RatePeriod>>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
}

impl TariffBuilder {
    pub fn daily_supply(mut self, rate: Decimal) -> Self {
        self.supply_rate = rate;
        self
    }

    pub fn rate_period<F>(mut self, f: F) -> Result<Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.import_tariff.push(builder.build());

        Ok(self)
    }

    pub fn export_rate_period<F>(mut self, f: F) -> Result<Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.export_tariff
            .get_or_insert_with(Vec::new)
            .push(builder.build());

        Ok(self)
    }

    pub fn build(self) -> Result<Tariff, TariffError> {
        let mut tariff = Tariff {
            import_tariff: self.import_tariff,
            export_tariff: self.export_tariff,
            supply_rate: self.supply_rate,
            discount: self.discount,
        };

        tariff.validate()?;
        Ok(tariff)
    }
}
#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_build_flat_period() {
        let rate_period = RatePeriodBuilder::default()
            .rates(&[(dec!(0.2), dec!(0))])
            .build();

        assert_eq!(rate_period.rates[0].rate, dec!(0.2));
        assert_eq!(
            rate_period.time_band.end(),
            HourMinute::new(23, 59).unwrap()
        );
    }

    #[test]
    fn test_build_single_rate_tou_period() {
        let rate_period = RatePeriodBuilder::default()
            .rates(&vec![(dec!(0.2), dec!(0))])
            .time_band(
                HourMinute::from_str("16:00").unwrap(),
                HourMinute::from_str("20:00").unwrap(),
                None,
            )
            .unwrap()
            .build();

        assert_eq!(rate_period.rates[0].rate, dec!(0.2));
    }

    #[test]
    fn test_multi_tiers_success() {
        let tariff = TariffBuilder::default()
            .daily_supply(dec!(0.1))
            .rate_period(|s| {
                s.rates(&[(dec!(0.1), dec!(0)), (dec!(0.2), dec!(1000))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(tariff.import_tariff[0].rates.len(), 2);
        assert_eq!(
            tariff.import_tariff[0].rates[1],
            RateBlock {
                rate: dec!(0.2),
                lower_band: dec!(1000)
            }
        );
    }

    #[test]
    fn test_threshold_tiers() {
        let tariff = TariffBuilder::default()
            .daily_supply(dec!(0.2))
            .rate_period(|b| {
                b.rates(&[(dec!(0.1), dec!(1000)), (dec!(0.2), dec!(500))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build();

        assert!(matches!(tariff, Err(TariffError::TierOrder)));
    }

    #[test]
    fn test_rate_period_rejects_invalid_band() {
        let result = TariffBuilder::default()
            .rate_period(|s| s.time_band(HourMinute::max(), HourMinute::min(), None)); // start > end
        assert!(matches!(result, Err(TariffError::InvalidTimeBand(_))));
    }

    #[test]
    fn test_band_gap() {
        let tariff = TariffBuilder::default()
            .rate_period(|x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("16:00").unwrap(),
                    HourMinute::from_str("20:00").unwrap(),
                    None,
                )
            })
            .unwrap()
            .build();

        assert!(matches!(tariff, Err(TariffError::Gap(_))));
    }

    #[test]
    fn test_overlap() {
        let tariff = TariffBuilder::default()
            .rate_period(|x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("00:00").unwrap(),
                    HourMinute::from_str("20:00").unwrap(),
                    None,
                )
            })
            .unwrap()
            .rate_period(|x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("16:00").unwrap(),
                    HourMinute::from_str("23:59").unwrap(),
                    None,
                )
            })
            .unwrap()
            .build();

        assert!(matches!(tariff, Err(TariffError::Overlap(_))));
    }

    #[test]
    fn test_export_tariff() {
        let tariff = TariffBuilder::default()
            .rate_period(|x| {
                x.rates(&[(dec!(0.3), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap()
            .export_rate_period(|x| {
                x.rates(&[(dec!(0.05), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(tariff.export_tariff.unwrap().len(), 1);
    }

    #[test]
    fn test_missing_rates() {
        let result = TariffBuilder::default()
            .rate_period(|b| b.time_band(HourMinute::min(), HourMinute::max(), None))
            .unwrap()
            .build();

        assert!(matches!(result, Err(TariffError::EmptyRates)));
    }
}
