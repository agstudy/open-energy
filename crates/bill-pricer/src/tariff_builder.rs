use rust_decimal::Decimal;

use crate::models::{HourMinute, InvalidTimeBand, TimeBand, WeekDays};

use crate::tariff::{Discount, RateBlock, RatePeriod, Tariff};

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
    ) -> Result<Self, InvalidTimeBand> {
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

#[derive(Debug, thiserror::Error)]
pub enum TariffError {
    #[error("gap in time band coverage")]
    TimeBandGapError,
    #[error("overlapping time bands")]
    OverlapTimeBandError,
    #[error("tier thresholds out of order")]
    ThresholdTierError,
    #[error("Missing rates")]
    MissingRatesError,
}

impl TariffBuilder {
    pub fn supply(mut self, rate: Decimal) -> Self {
        self.supply_rate = rate;
        self
    }

    /// Push a rate period, configured via a closure on its own builder.
    pub fn rate_period<F>(mut self, f: F) -> Result<Self, InvalidTimeBand>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, InvalidTimeBand>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.import_tariff.push(builder.build());

        Ok(self)
    }

    /// Push a rate period, configured via a closure on its own builder.
    pub fn export_rate_period<F>(mut self, f: F) -> Result<Self, InvalidTimeBand>
    where
        F: FnOnce(RatePeriodBuilder) -> Result<RatePeriodBuilder, InvalidTimeBand>,
    {
        let builder = f(RatePeriodBuilder::default())?;
        self.export_tariff
            .get_or_insert_with(Vec::new)
            .push(builder.build());

        Ok(self)
    }

    pub fn check_tier_order(rates: &[RateBlock]) -> Result<(), TariffError> {
        let bands: Vec<_> = rates.iter().map(|v| v.lower_band).collect();

        let Some((&first, _)) = bands.split_first() else {
            return Err(TariffError::MissingRatesError);
        };

        if first != Decimal::ZERO {
            return Err(TariffError::ThresholdTierError);
        }

        for w in bands.windows(2) {
            let (current, next) = (&w[0], &w[1]);
            if current >= next {
                return Err(TariffError::ThresholdTierError);
            }
        }
        Ok(())
    }
    /*
    1. sort tp by s_i                      -- O(n log n)
    2. cursor ← 0
    3. for each (s_i, e_i) in sorted order:
       if s_i > cursor:  return Gap(cursor)       -- hole before this band
       if s_i < cursor:  return Overlap(s_i)       -- band starts before prev ended
       cursor ← e_i + 1
    4. if cursor ≠ 1440:  return Gap(cursor)            -- day not fully covered
    5. return Ok
    */
    pub fn validate_tariff(rps: &mut [RatePeriod]) -> Result<(), TariffError> {
        rps.sort_by_key(|rp| rp.time_band.start());

        let mut cursor: u32 = 0;
        for rp in rps.iter() {
            Self::check_tier_order(&rp.rates)?;
            let tb = &rp.time_band;
            if tb.start().total_minutes() > cursor {
                return Err(TariffError::TimeBandGapError);
            } else if tb.start().total_minutes() < cursor {
                return Err(TariffError::OverlapTimeBandError);
            }
            // `end()` is inclusive, so the next band must start at end+1 to be gap-free.
            cursor = tb.end().total_minutes() + 1;
        }
        if cursor != 1440 {
            return Err(TariffError::TimeBandGapError);
        }

        Ok(())
    }

    pub fn build(mut self) -> Result<Tariff, TariffError> {
        Self::validate_tariff(&mut self.import_tariff)?;
        if let Some(export) = &mut self.export_tariff {
            Self::validate_tariff(export)?;
        }

        Ok(Tariff {
            import_tariff: self.import_tariff,
            export_tariff: self.export_tariff,
            supply_rate: self.supply_rate,
            discount: self.discount,
        })
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
    fn test_build_creation() {
        let tariff = TariffBuilder::default()
            .supply(dec!(0.1))
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
            .supply(dec!(0.2))
            .rate_period(|b| {
                b.rates(&[(dec!(0.1), dec!(1000)), (dec!(0.2), dec!(500))])
                    .time_band(HourMinute::min(), HourMinute::max(), None)
            })
            .unwrap()
            .build();

        assert!(matches!(tariff, Err(TariffError::ThresholdTierError)));
    }

    #[test]
    fn test_rate_period_rejects_invalid_band() {
        let result = TariffBuilder::default()
            .rate_period(|s| s.time_band(HourMinute::max(), HourMinute::min(), None)); // start > end
        assert!(matches!(result, Err(InvalidTimeBand)));
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

        assert!(matches!(tariff, Err(TariffError::TimeBandGapError)));
    }
}
