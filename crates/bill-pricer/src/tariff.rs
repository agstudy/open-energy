use std::cmp::min;

use crate::models::HourMinute;
use crate::models::{InvalidTimeBand, TimeBand};
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::models::WeekDays;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
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

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RatePeriod {
    pub rates: Vec<RateBlock>,
    pub time_band: TimeBand,
}

impl RatePeriod {
    pub fn applies_at(&self, at: &HourMinute) -> bool {
        self.time_band.start() <= *at && *at <= self.time_band.end()
    }

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
#[serde(try_from = "TariffRaw")]
pub struct Tariff {
    import_tariff: Vec<RatePeriod>,
    export_tariff: Option<Vec<RatePeriod>>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
}

#[derive(Deserialize)]
pub struct TariffRaw {
    import_tariff: Vec<RatePeriod>,
    export_tariff: Option<Vec<RatePeriod>>,
    discount: Option<Vec<Discount>>,
    supply_rate: Decimal,
}

impl TryFrom<TariffRaw> for Tariff {
    type Error = TariffError;
    fn try_from(raw: TariffRaw) -> Result<Self, TariffError> {
        let mut tariff = Tariff {
            import_tariff: raw.import_tariff,
            export_tariff: raw.export_tariff,
            supply_rate: raw.supply_rate,
            discount: raw.discount,
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
    #[error("bands leave a gap starting at {0}")]
    Gap(HourMinute),
    #[error("There is no matching rate period covering: {0}")]
    NoMatchingPeriod(HourMinute),
    #[error("bands overlap at {0}")]
    Overlap(HourMinute),
    #[error("tier thresholds out of order")]
    TierOrder,
    #[error("Missing rates")]
    EmptyRates,
    #[error(transparent)]
    InvalidTimeBand(#[from] InvalidTimeBand),
}

impl Tariff {
    pub fn rate_at(&self, at: DateTime<Utc>) -> Result<Decimal, TariffError> {
        let at_hm =
            HourMinute::new(at.hour(), at.minute()).expect("chrono guarantees valid hour/minute");

        let matches: Vec<&RatePeriod> = self
            .import_tariff
            .iter()
            .filter(|p| p.applies_at(&at_hm))
            .collect();

        match matches.as_slice() {
            [] => Err(TariffError::NoMatchingPeriod(at_hm)),
            [period] => period
                .rates
                .first()
                .ok_or(TariffError::EmptyRates)
                .map(|b| b.rate),
            _ => Err(TariffError::Overlap(at_hm)),
        }
    }

    pub fn validate(&mut self) -> Result<(), TariffError> {
        validate_tariff(&mut self.import_tariff)?;
        if let Some(export) = &mut self.export_tariff {
            validate_tariff(export)?;
        }
        Ok(())
    }
    pub fn supply_rate(&self) -> Decimal {
        self.supply_rate
    }
}

/// Validate that a set of rate periods fully covers a day with no overlaps
/// and monotonic tier thresholds.
///
/// ```text
/// 1. sort tp by s_i                      -- O(n log n)
/// 2. cursor ← 0
/// 3. for each (s_i, e_i) in sorted order:
///      check_tier_order(rates_i)?
///      if s_i > cursor:  return Gap(cursor)
///      if s_i < cursor:  return Overlap(s_i)
///      cursor ← e_i + 1
/// 4. if cursor ≠ 1440: return Gap(cursor)
/// 5. return Ok
/// ```
fn validate_tariff(rps: &mut [RatePeriod]) -> Result<(), TariffError> {
    rps.sort_by_key(|rp| rp.time_band.start());

    let mut cursor: u32 = 0;
    for rp in rps.iter() {
        check_tier_order(&rp.rates)?;

        let start = rp.time_band.start().minute_of_day();
        let end = rp.time_band.end().minute_of_day();

        if start > cursor {
            return Err(TariffError::Gap(from_minute_of_day_unchecked(cursor)));
        } else if start < cursor {
            return Err(TariffError::Overlap(from_minute_of_day_unchecked(min(
                cursor, 1439,
            ))));
        }
        cursor = end + 1;
    }

    if cursor != 1440 {
        return Err(TariffError::Gap(from_minute_of_day_unchecked(min(
            cursor, 1439,
        ))));
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
        return Err(TariffError::TierOrder);
    }

    let mut prev = first;
    for cur in it {
        if prev >= cur {
            return Err(TariffError::TierOrder);
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
