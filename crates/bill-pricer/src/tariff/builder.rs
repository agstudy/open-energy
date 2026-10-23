use domain::{HourMinute, year_month::SeasonRange};
use rust_decimal::Decimal;

use crate::{
    TimeBand, WeekDays,
    tariff::{
        ConsumptionPeriod, Discount, RateBlock, RatePeriod, Tariff, TariffError, TariffSeason,
    },
};

pub struct RatePeriodBuilder {
    rates: Vec<RateBlock>,
    time_band: TimeBand,
}

impl Default for RatePeriodBuilder {
    fn default() -> Self {
        Self {
            rates: Vec::default(),
            time_band: TimeBand::full_day(),
        }
    }
}

fn rates_mapper(rates: &[(Decimal, Decimal)]) -> Vec<RateBlock> {
    rates
        .iter()
        .map(|(r, b)| RateBlock {
            rate: *r,
            lower_band: *b,
        })
        .collect()
}

impl RatePeriodBuilder {
    #[must_use]
    pub fn rates(mut self, block_rates: &[(Decimal, Decimal)]) -> Self {
        self.rates = rates_mapper(block_rates);
        self
    }

    /// Sets the time band for the rate period being built.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError::InvalidTimeBand`] if `start` is after `end`,
    /// or if the band is otherwise rejected by [`TimeBand::new`].
    pub fn time_band(
        mut self,
        start: HourMinute,
        end: HourMinute,
        days_of_week: Option<WeekDays>,
    ) -> Result<Self, TariffError> {
        self.time_band = TimeBand::new(start, end, days_of_week)?;
        Ok(self)
    }

    #[must_use]
    pub fn build(self) -> RatePeriod {
        RatePeriod {
            rates: self.rates,
            time_band: self.time_band,
        }
    }
}

#[derive(Default)]
pub struct TariffBuilder {
    import: Vec<TariffSeason>,
    export: Vec<TariffSeason>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
}

impl TariffBuilder {
    pub fn consumption_period(&mut self, cons_period: Option<ConsumptionPeriod>) {
        self.cons_period = cons_period;
    }

    pub fn daily_supply(&mut self, rate: Decimal) {
        self.supply_rate = rate;
    }

    fn season_mut_or_insert(
        seasons: &mut Vec<TariffSeason>,
        season: SeasonRange,
    ) -> &mut TariffSeason {
        if let Some(idx) = seasons.iter().position(|s| s.season == season) {
            return &mut seasons[idx];
        }
        seasons.push(TariffSeason {
            periods: Vec::new(),
            season,
        });
        seasons.last_mut().expect("just pushed")
    }

    /// Adds an import rate period using the given builder closure.
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`] if `f` returns an error.
    pub fn rate_period<F>(&mut self, season: SeasonRange, f: F) -> Result<&mut Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let period = f(RatePeriodBuilder::default())?.build(); // may fail — no mutation yet
        Self::season_mut_or_insert(&mut self.import, season)
            .periods
            .push(period);

        Ok(self)
    }

    /// Adds an import rate period using direct `RatePeriod` parameters
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    pub fn add_rate_period(
        &mut self,
        season: SeasonRange,
        rates: &[(Decimal, Decimal)],
        time_band: TimeBand,
    ) -> &mut Self {
        let period = RatePeriod {
            rates: rates_mapper(rates),
            time_band,
        };

        Self::season_mut_or_insert(&mut self.import, season)
            .periods
            .push(period);

        self
    }

    /// Adds an export rate period using direct `RatePeriod` parameters
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    pub fn add_export_rate_period(
        &mut self,
        season: SeasonRange,
        rates: &[(Decimal, Decimal)],
        time_band: TimeBand,
    ) -> &mut Self {
        let period = RatePeriod {
            rates: rates_mapper(rates),
            time_band,
        };

        Self::season_mut_or_insert(&mut self.export, season)
            .periods
            .push(period);

        self
    }

    /// Adds an export (feed-in) rate period using the given builder closure.
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`] if `f` returns an error.
    pub fn export_rate_period<F>(
        &mut self,
        season: SeasonRange,
        f: F,
    ) -> Result<&mut Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let period = f(RatePeriodBuilder::default())?.build(); // may fail — no mutation yet
        Self::season_mut_or_insert(&mut self.export, season)
            .periods
            .push(period);

        Ok(self)
    }
    /// Finalises and returns the built [`Tariff`].
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`] if validation fails — for example
    /// [`TariffError::Gap`] if a day is not fully covered,
    /// [`TariffError::Overlap`] if bands overlap,
    /// [`TariffError::TierOrder`] if tier thresholds are not strictly
    /// increasing from zero, or [`TariffError::EmptyRates`] if a period
    /// has no rate blocks.
    pub fn build(self) -> Result<Tariff, TariffError> {
        let tariff = Tariff::new_unchecked(
            self.import,
            self.export,
            self.discount,
            self.supply_rate,
            self.cons_period,
        );
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
            .rates(&[(dec!(0.2), dec!(0))])
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
        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(0.1));
        builder
            .rate_period(SeasonRange::full_year(), |s| {
                s.rates(&[(dec!(0.1), dec!(0)), (dec!(0.2), dec!(1000))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        assert_eq!(tariff.import_tariff()[0].periods[0].rates.len(), 2);
        assert_eq!(
            tariff.import_tariff()[0].periods[0].rates[1],
            RateBlock {
                rate: dec!(0.2),
                lower_band: dec!(1000)
            }
        );
    }

    #[test]
    fn test_threshold_tiers() {
        let mut builder = TariffBuilder::default();
        builder.daily_supply(dec!(0.2));
        builder
            .rate_period(SeasonRange::full_year(), |b| {
                b.rates(&[(dec!(0.1), dec!(1000))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap();
        let tariff = builder.build();

        match tariff {
            Err(TariffError::TierOrder(prev, cur)) => {
                assert_eq!(prev, dec!(1000));
                assert_eq!(cur, Decimal::ZERO);
            }
            _ => panic!("expceted TierOrder, got error"),
        }
    }

    #[test]
    fn test_rate_period_rejects_invalid_band() {
        let mut builder = TariffBuilder::default();
        let result = builder.rate_period(SeasonRange::full_year(), |s| {
            s.time_band(HourMinute::max(), HourMinute::min(), None)
        }); // start > end
        assert!(matches!(result, Err(TariffError::InvalidTimeBand(_))));
    }

    #[test]
    fn test_band_gap() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(SeasonRange::full_year(), |x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("16:00").unwrap(),
                    HourMinute::from_str("20:00").unwrap(),
                    None,
                )
            })
            .unwrap();

        assert!(matches!(builder.build(), Err(TariffError::Gap(_, _))));
    }

    #[test]
    fn test_overlap() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(SeasonRange::full_year(), |x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("00:00").unwrap(),
                    HourMinute::from_str("20:00").unwrap(),
                    None,
                )
            })
            .unwrap();
        builder
            .rate_period(SeasonRange::full_year(), |x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("16:00").unwrap(),
                    HourMinute::from_str("23:59").unwrap(),
                    None,
                )
            })
            .unwrap();

        assert!(matches!(builder.build(), Err(TariffError::Overlap(_, _))));
    }

    #[test]
    fn test_export_tariff() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(SeasonRange::full_year(), |x| {
                x.rates(&[(dec!(0.3), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap();
        builder
            .export_rate_period(SeasonRange::full_year(), |x| {
                x.rates(&[(dec!(0.05), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        assert_eq!(tariff.export_tariff().len(), 1);
    }

    #[test]
    fn test_missing_rates() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(SeasonRange::full_year(), |b| {
                b.time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();

        assert!(matches!(builder.build(), Err(TariffError::EmptyRates)));
    }
}
