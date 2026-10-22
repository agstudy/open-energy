use std::cmp::min;

use chrono::Datelike;
use chrono::{NaiveDate, Weekday};
use domain::{
    HourMinute,
    hour_minute::InvalidStrHourMinute,
    year_month::{MonthDay, SeasonRange},
};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::{InvalidTimeBand, TimeBand, WeekDays};

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Decimal,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountValueType {
    Percent,
    Absolute,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountApplicationType {
    Total,
    Energy,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Discount {
    pub value_type: DiscountValueType,
    pub application_type: DiscountApplicationType,
    pub amount: Decimal,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
pub enum ConsumptionPeriod {
    Day,
    Month,
    BiMonthly,
    Quarterly,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RatePeriod {
    pub rates: Vec<RateBlock>,
    pub time_band: TimeBand,
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct TariffSeason {
    pub rates: Vec<RatePeriod>,
    pub season: SeasonRange,
}

#[derive(Debug, thiserror::Error)]
pub enum SeasonError {
    #[error("seasons have gap at {0}")]
    Gap(MonthDay),
    #[error("seasons overlaps at {0}")]
    Overlap(MonthDay),
    #[error("no seasons defined")]
    Empty,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum EmptyBehavior {
    Allow,
    Reject,
}
/// Check full year for tariff seasons
///
/// This function will be used for mondatory/optioan tariff sutructures
/// import is mondtaory , export is less
/// The use of `empty_behavior` is to allow or reject this behavior
///
/// # Errors
///
/// returns [`SeasonError::Gap`] if there a day date not covered
/// returns [`SeasonError::Overlap`] if there a day date covered multi times
pub fn check_full_year(
    seasons: &[SeasonRange],
    empty_behavior: &EmptyBehavior,
) -> Result<(), SeasonError> {
    const ORIGIN: NaiveDate = match NaiveDate::from_ymd_opt(2017, 1, 1) {
        Some(d) => d,
        None => panic!("2017-01-01 is a valid date"),
    };
    if seasons.is_empty() {
        match empty_behavior {
            EmptyBehavior::Reject => return Err(SeasonError::Empty),
            EmptyBehavior::Allow => return Ok(()),
        }
    }

    let origin = ORIGIN;
    for day in origin.iter_days().take(365) {
        let md = MonthDay::new_unchecked(day.month(), day.day());
        let count = seasons.iter().filter(|s| s.contains(md)).count();
        if count == 0 {
            return Err(SeasonError::Gap(md));
        }
        if count > 1 {
            return Err(SeasonError::Overlap(md));
        }
    }

    Ok(())
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(try_from = "TariffRaw")]
pub struct Tariff {
    import: Vec<RatePeriod>,
    export: Vec<RatePeriod>,
    discount: Option<Vec<Discount>>, // todo: apply discounts
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
}

#[derive(Deserialize)]
pub struct TariffRaw {
    import: Vec<RatePeriod>,
    export: Vec<RatePeriod>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
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

#[derive(Debug, thiserror::Error)]
pub enum PricingError {
    #[error("no meter data provided")]
    NoData,
    #[error(transparent)]
    Tariff(#[from] TariffError),
}

#[derive(Debug, thiserror::Error)]
pub enum TariffError {
    #[error("bands leave a gap starting at day {0} at {1}")]
    Gap(Weekday, HourMinute),
    #[error("There is no matching rate period covering: {0}")]
    NoMatchingPeriod(Weekday, HourMinute),
    #[error("bands overlap at {0}")]
    Overlap(Weekday, HourMinute),
    #[error("tier thresholds out of order: prev {0} cur {1}")]
    TierOrder(Decimal, Decimal),
    #[error("Missing rates")]
    EmptyRates,
    #[error(transparent)]
    InvalidTimeBand(#[from] InvalidTimeBand),
    #[error(transparent)]
    InvalidStrHourMinute(#[from] InvalidStrHourMinute),
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
impl Tariff {
    /// Returns the rate period of the given direction that applies at a time of day and weekday.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError::NoMatchingPeriod`] if no period covers `hm` on `weekday`,
    /// and [`TariffError::Overlap`] if more than one does.
    pub fn period_at(
        &self,
        direction: TariffDirection,
        hm: HourMinute,
        weekday: Weekday,
    ) -> Result<&RatePeriod, TariffError> {
        let periods = match direction {
            TariffDirection::Import => self.import_tariff(),
            TariffDirection::Export => self.export_tariff(),
        };
        let mut matches = periods.iter().filter(|p| p.applies_at(&hm, weekday));
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
        direction: TariffDirection,
        hm: HourMinute,
        weekday: Weekday,
        consumed_before: Decimal,
        delta: Decimal,
    ) -> Result<Decimal, TariffError> {
        let period = self.period_at(direction, hm, weekday)?;
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
        if !self.import.is_empty() {
            for (day, rps) in group_per_weekday(&self.import) {
                validate_tariff(day, &rps)?;
            }
        }
        if !self.export.is_empty() {
            for (day, rps) in group_per_weekday(&self.export) {
                validate_tariff(day, &rps)?;
            }
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

    #[must_use]
    pub fn import_tariff(&self) -> &[RatePeriod] {
        self.import.as_slice()
    }

    #[must_use]
    pub fn export_tariff(&self) -> &[RatePeriod] {
        self.export.as_slice()
    }
}

fn group_per_weekday(rps: &[RatePeriod]) -> [(Weekday, Vec<&RatePeriod>); 7] {
    [
        Weekday::Mon,
        Weekday::Tue,
        Weekday::Wed,
        Weekday::Thu,
        Weekday::Fri,
        Weekday::Sat,
        Weekday::Sun,
    ]
    .map(|weekday| {
        let mut day_rps: Vec<&RatePeriod> = rps
            .iter()
            .filter(|rp| {
                rp.time_band
                    .days_of_week()
                    .is_none_or(|days| days.has(weekday))
            })
            .collect();
        day_rps.sort_by_key(|rp| rp.time_band.start());
        (weekday, day_rps)
    })
}
/// Validate that a set of rate periods fully covers a day with no overlaps
/// and monotonic tier thresholds.
///
/// ```text
/// 2. cursor ← 0
/// 3. for each (s_i, e_i) in sorted order:
///      check_tier_order(rates_i)?
///      if s_i > cursor:  return Gap(cursor)
///      if s_i < cursor:  return Overlap(s_i)
///      cursor ← e_i + 1
/// 4. if cursor ≠ 1440: return Gap(cursor)
/// 5. return Ok
/// ```
fn validate_tariff(day: Weekday, rps: &[&RatePeriod]) -> Result<(), TariffError> {
    let hm = |val| from_minute_of_day_unchecked(val);
    let mut cursor: u32 = 0;
    for rp in rps {
        check_tier_order(&rp.rates)?;

        let start = rp.time_band.start().minute_of_day();
        let end = rp.time_band.end().minute_of_day();

        if start > cursor {
            return Err(TariffError::Gap(day, hm(cursor)));
        } else if start < cursor {
            return Err(TariffError::Overlap(day, hm(min(cursor, 1439))));
        }
        cursor = end + 1;
    }

    if cursor != 1440 {
        return Err(TariffError::Gap(day, hm(min(cursor, 1439))));
    }

    Ok(())
}

fn from_minute_of_day_unchecked(m: u32) -> HourMinute {
    HourMinute::from_minute_of_day(m).expect("minute-of-day out of range")
}

fn check_tier_order(rates: &[RateBlock]) -> Result<(), TariffError> {
    let mut it = rates.iter().map(|v| v.lower_band);

    let Some(first) = it.next() else {
        return Err(TariffError::EmptyRates);
    };

    if first != Decimal::ZERO {
        return Err(TariffError::TierOrder(first, Decimal::ZERO));
    }

    let mut prev = first;
    for cur in it {
        if prev >= cur {
            return Err(TariffError::TierOrder(prev, cur));
        }
        prev = cur;
    }
    Ok(())
}

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
    import: Vec<RatePeriod>,
    export: Vec<RatePeriod>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
    cons_period: Option<ConsumptionPeriod>,
}

impl TariffBuilder {
    #[must_use]
    pub fn consumption_period(mut self, cons_period: Option<ConsumptionPeriod>) -> Self {
        self.cons_period = cons_period;
        self
    }

    #[must_use]
    pub fn daily_supply(mut self, rate: Decimal) -> Self {
        self.supply_rate = rate;
        self
    }
    /// Adds an import rate period using the given builder closure.
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`] if `f` returns an error.
    pub fn rate_period<F>(&mut self, f: F) -> Result<&mut Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.import.push(builder.build());

        Ok(self)
    }

    /// Adds an import rate period using direct `RatePeriod` parameters
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    #[must_use]
    pub fn add_rate_period(mut self, rates: &[(Decimal, Decimal)], time_band: TimeBand) -> Self {
        self.import.push(RatePeriod {
            rates: rates_mapper(rates),
            time_band,
        });
        self
    }

    /// Adds an impoexport rt rate period using direct `RatePeriod` parameters
    ///
    /// Overlap and coverage are not checked here; they are validated by
    /// [`TariffBuilder::build`].
    pub fn add_export_rate_period(
        &mut self,
        rates: &[(Decimal, Decimal)],
        time_band: TimeBand,
    ) -> &mut Self {
        self.export.push(RatePeriod {
            rates: rates_mapper(rates),
            time_band,
        });
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
    pub fn export_rate_period<F>(&mut self, f: F) -> Result<&mut Self, TariffError>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, TariffError>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.export.push(builder.build());

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
        let tariff = Tariff {
            import: self.import,
            export: self.export,
            supply_rate: self.supply_rate,
            discount: self.discount,
            cons_period: self.cons_period,
        };
        tariff.validate()?;
        Ok(tariff)
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rust_decimal_macros::dec;

    use crate::tariff_factory::TariffFactory;

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
        let mut builder = TariffBuilder::default().daily_supply(dec!(0.1));
        builder
            .rate_period(|s| {
                s.rates(&[(dec!(0.1), dec!(0)), (dec!(0.2), dec!(1000))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        assert_eq!(tariff.import[0].rates.len(), 2);
        assert_eq!(
            tariff.import[0].rates[1],
            RateBlock {
                rate: dec!(0.2),
                lower_band: dec!(1000)
            }
        );
    }

    #[test]
    fn test_threshold_tiers() {
        let mut builder = TariffBuilder::default().daily_supply(dec!(0.2));
        builder
            .rate_period(|b| {
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
        let result =
            builder.rate_period(|s| s.time_band(HourMinute::max(), HourMinute::min(), None)); // start > end
        assert!(matches!(result, Err(TariffError::InvalidTimeBand(_))));
    }

    #[test]
    fn test_band_gap() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(|x| {
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
            .rate_period(|x| {
                x.rates(&[(dec!(1.0), dec!(0))]).time_band(
                    HourMinute::from_str("00:00").unwrap(),
                    HourMinute::from_str("20:00").unwrap(),
                    None,
                )
            })
            .unwrap();
        builder
            .rate_period(|x| {
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
            .rate_period(|x| {
                x.rates(&[(dec!(0.3), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap();
        builder
            .export_rate_period(|x| {
                x.rates(&[(dec!(0.05), dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })
            .unwrap();
        let tariff = builder.build().unwrap();

        assert_eq!(tariff.export.len(), 1);
    }

    #[test]
    fn test_missing_rates() {
        let mut builder = TariffBuilder::default();
        builder
            .rate_period(|b| b.time_band(HourMinute::min(), HourMinute::max(), None))
            .unwrap();

        assert!(matches!(builder.build(), Err(TariffError::EmptyRates)));
    }

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
            tariff
                .cost_at(
                    TariffDirection::Import,
                    HourMinute::new_unchecked(1, 0),
                    Weekday::Fri,
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
            tariff
                .cost_at(
                    TariffDirection::Import,
                    HourMinute::new_unchecked(1, 0),
                    Weekday::Fri,
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
