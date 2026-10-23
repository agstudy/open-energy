use chrono::Weekday;
use domain::{
    HourMinute,
    year_month::{MonthDay, SeasonRange},
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::TimeBand;
use crate::tariff::validate::{EmptyBehavior, TariffChecker};
use crate::tariff::{
    ConsumptionPeriod, Discount, RateBlock, RatePeriod, SeasonError, TariffError, TariffSeason,
};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(try_from = "TariffRaw")]
pub struct Tariff {
    import: Vec<TariffSeason>,
    export: Vec<TariffSeason>,
    discount: Option<Vec<Discount>>, // todo: apply discounts
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
}

#[derive(Deserialize)]
pub struct TariffRaw {
    import: Vec<TariffSeason>,
    export: Vec<TariffSeason>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
}

impl RatePeriod {
    #[must_use]
    pub fn applies_at(&self, at: &HourMinute, weekday: Weekday) -> bool {
        let in_range = self.time_band.start() <= *at && *at <= self.time_band.end();
        let on_day = self
            .time_band
            .days_of_week()
            .is_none_or(|value| value.has(weekday));
        in_range && on_day
    }

    #[must_use]
    pub fn new(rate: Decimal, time_band: TimeBand) -> Self {
        RatePeriod {
            rates: vec![RateBlock {
                rate,
                lower_band: dec!(0.0),
            }],
            time_band,
        }
    }
}

impl TryFrom<TariffRaw> for Tariff {
    type Error = TariffError;
    fn try_from(raw: TariffRaw) -> Result<Self, TariffError> {
        let tariff = Tariff {
            import: raw.import,
            export: raw.export,
            supply_rate: raw.supply_rate,
            discount: raw.discount,
            cons_period: raw.cons_period,
        };

        tariff.validate()?;
        Ok(tariff)
    }
}

fn cumulative_cost(rates: &[RateBlock], up_to: Decimal) -> Decimal {
    let mut cost = Decimal::ZERO;

    for (i, band) in rates.iter().enumerate() {
        let lower = band.lower_band;
        if lower >= up_to {
            break;
        }
        let upper = rates.get(i + 1).map_or(up_to, |b| b.lower_band.min(up_to));
        cost += (upper - lower) * band.rate;
    }
    cost
}

#[derive(Debug, Copy, Clone)]
pub enum TariffDirection {
    Import,
    Export,
}

pub struct Instant {
    pub md: MonthDay,
    pub hm: HourMinute,
    pub weekday: Weekday,
}

impl Tariff {
    pub(crate) fn new_unchecked(
        import: Vec<TariffSeason>,
        export: Vec<TariffSeason>,
        discount: Option<Vec<Discount>>,
        supply_rate: Decimal,
        cons_period: Option<ConsumptionPeriod>,
    ) -> Self {
        Self {
            import,
            export,
            discount,
            supply_rate,
            cons_period,
        }
    }

    /// Returns the rate period of the given direction that applies at a time of day and weekday.
    ///
    /// # Errors
    ///
    /// Returns [`SeasonError::NoMatchingPeriod`] if no period covers `hm` on `weekday`,
    /// and [`TariffError::Overlap`] if more than one does.
    pub fn period_at(
        &self,
        direction: TariffDirection,
        instant: &Instant,
    ) -> Result<&RatePeriod, TariffError> {
        self.find_season(direction, instant.md)?
            .period_at(instant.hm, instant.weekday)
    }

    /// Returns the cost of consuming `delta` kWh in the period that applies at `hm` and `weekday`,
    /// given `consumed_before` kWh already counted in the current reset period.
    ///
    /// The cost is the difference between the cumulative tier cost after and before the reading,
    /// so a reading that crosses one or more tier thresholds is priced across them. The result is
    /// not rounded.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Tariff::period_at`].
    pub fn cost_at(
        &self,
        direction: TariffDirection,
        instant: &Instant,
        consumed_before: Decimal,
        delta: Decimal,
    ) -> Result<Decimal, TariffError> {
        let tariff_season = self.find_season(direction, instant.md)?;
        tariff_season.cost_at(instant.hm, instant.weekday, consumed_before, delta)
    }

    /// # Errors
    ///
    /// Returns [`TariffError::Gap`] if a day is not fully covered,
    /// [`TariffError::Overlap`] if bands overlap, [`TariffError::TierOrder`]
    /// if tier thresholds are not strictly increasing from zero, or
    /// [`TariffError::EmptyRates`] if a period has no rate blocks.
    /// it can also return [`TariffError::Season(SeasonError::Gap(md))`].
    pub fn validate(&self) -> Result<(), TariffError> {
        TariffChecker::check_full_year(self.import_tariff(), &EmptyBehavior::Reject)?;
        for tariff_season in &self.import {
            tariff_season.validate()?;
        }

        TariffChecker::check_full_year(self.import_tariff(), &EmptyBehavior::Allow)?;
        for tariff_season in &self.export {
            tariff_season.validate()?;
        }
        Ok(())
    }
    #[must_use]
    pub fn supply_rate(&self) -> Decimal {
        self.supply_rate
    }

    #[must_use]
    pub fn consumption_period(&self) -> Option<ConsumptionPeriod> {
        self.cons_period
    }

    /// Gets the current season for md.
    ///
    /// # Errors
    ///
    /// returns [`SeasonError::NoSeasonForDate`] if no season contains md
    pub fn find_season(
        &self,
        direction: TariffDirection,
        md: MonthDay,
    ) -> Result<&TariffSeason, SeasonError> {
        let tariffs = match direction {
            TariffDirection::Import => self.import_tariff(),
            TariffDirection::Export => self.export_tariff(),
        };

        tariffs
            .iter()
            .find(|s| s.season.contains(md))
            .ok_or(SeasonError::NoSeasonForDate(md))
    }

    #[must_use]
    pub fn import_tariff(&self) -> &[TariffSeason] {
        &self.import
    }

    #[must_use]
    pub fn export_tariff(&self) -> &[TariffSeason] {
        &self.export
    }
    #[must_use]
    pub fn get_seasons(&self, direction: TariffDirection) -> Vec<SeasonRange> {
        match direction {
            TariffDirection::Import => self.import.iter().map(|a| a.season).collect(),
            TariffDirection::Export => self.export.iter().map(|a| a.season).collect(),
        }
    }
}

impl TariffSeason {
    /// Returns the rate period of the given direction that applies at a time of day and weekday.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError::NoMatchingPeriod`] if no period covers `hm` on `weekday`,
    /// and [`TariffError::Overlap`] if more than one does.
    pub fn period_at(&self, hm: HourMinute, weekday: Weekday) -> Result<&RatePeriod, TariffError> {
        let mut matches = self.periods.iter().filter(|p| p.applies_at(&hm, weekday));
        match (matches.next(), matches.next()) {
            (None, _) => Err(TariffError::NoMatchingPeriod(weekday, hm)),
            (Some(p), None) => Ok(p),
            (Some(_), Some(_)) => Err(TariffError::Overlap(weekday, hm)),
        }
    }

    /// Returns the cost of consuming `delta` kWh in the period that applies at `hm` and `weekday`,
    /// given `consumed_before` kWh already counted in the current reset period.
    ///
    /// The cost is the difference between the cumulative tier cost after and before the reading,
    /// so a reading that crosses one or more tier thresholds is priced across them. The result is
    /// not rounded.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Tariff::period_at`].
    pub fn cost_at(
        &self,
        hm: HourMinute,
        weekday: Weekday,
        consumed_before: Decimal,
        delta: Decimal,
    ) -> Result<Decimal, TariffError> {
        let period = self.period_at(hm, weekday)?;
        let start = cumulative_cost(&period.rates, consumed_before);
        let end = cumulative_cost(&period.rates, consumed_before + delta);
        Ok(end - start)
    }

    /// # Errors
    ///
    /// Returns [`TariffError::Gap`] if a day is not fully covered,
    /// [`TariffError::Overlap`] if bands overlap, [`TariffError::TierOrder`]
    /// if tier thresholds are not strictly increasing from zero, or
    /// [`TariffError::EmptyRates`] if a period has no rate blocks.
    pub fn validate(&self) -> Result<(), TariffError> {
        for (day, rps) in TariffChecker::group_per_weekday(&self.periods) {
            TariffChecker::validate_tariff_period(day, &rps)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use rust_decimal_macros::dec;

    use crate::tariff::TariffFactory;

    use super::*;

    #[test]
    fn test_cost_at_inside_one_tier() {
        let blocks: Vec<(Decimal, Decimal)> = [(0.2, 0.0), (1.0, 10.0), (2.0, 20.0)]
            .iter()
            .map(|&(a, b)| (Decimal::try_from(a).unwrap(), Decimal::try_from(b).unwrap()))
            .collect();
        let tariff = TariffFactory::multiflat(&blocks, dec!(1.0), None)
            .unwrap()
            .build()
            .unwrap();

        let cost = |b: u32, d: u32| -> Decimal {
            let instant = Instant {
                weekday: Weekday::Fri,
                hm: HourMinute::new_unchecked(1, 0),
                md: MonthDay::new_unchecked(1, 1),
            };
            tariff
                .cost_at(
                    TariffDirection::Import,
                    &instant,
                    Decimal::from(b),
                    Decimal::from(d),
                )
                .unwrap()
        };
        //Inside one tier: before 2, delta 3 : 3 * 0.2
        assert_eq!(cost(2, 3), dec!(0.6));
        //exactly at tier end
        assert_eq!(cost(8, 2), dec!(0.4));
        // starts at threshold
        assert_eq!(cost(10, 2), dec!(2));
        // crosses the threshold before
        assert_eq!(cost(9, 2), dec!(1.2));
        // crosses 2 thresholds before
        assert_eq!(cost(21, 2), dec!(4));
        // beyonnd the last tier
        assert_eq!(cost(100, 1), dec!(2));
        // delta =0
        assert_eq!(cost(100, 0), dec!(0));
        assert_eq!(cost(10, 0), dec!(0));
    }

    #[test]
    fn test_cost_at_flat() {
        let tariff = TariffFactory::flat(dec!(1), dec!(1.0)).unwrap();

        let cost = |b: u32, d: u32| -> Decimal {
            let instant = Instant {
                weekday: Weekday::Fri,
                hm: HourMinute::new_unchecked(1, 0),
                md: MonthDay::new_unchecked(1, 1),
            };
            tariff
                .cost_at(
                    TariffDirection::Import,
                    &instant,
                    Decimal::from(b),
                    Decimal::from(d),
                )
                .unwrap()
        };
        // flat_rate * delta
        assert_eq!(cost(100, 1), dec!(1));
        assert_eq!(cost(0, 10), dec!(10));
    }
}
