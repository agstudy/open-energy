use chrono::{NaiveDate, Weekday};
use domain::{HourMinute, year_month::MonthDay};
use rust_decimal::Decimal;
use std::cmp::min;

use crate::tariff::{RateBlock, RatePeriod, SeasonError, TariffError, TariffSeason};
use chrono::Datelike;

#[derive(Debug, PartialEq)]
pub enum EmptyBehavior {
    Allow,
    Reject,
}

pub(crate) struct TariffChecker;
impl TariffChecker {
    pub fn from_minute_of_day_unchecked(m: u32) -> HourMinute {
        HourMinute::from_minute_of_day(m).expect("minute-of-day out of range")
    }

    pub fn group_per_weekday(rps: &[RatePeriod]) -> [(Weekday, Vec<&RatePeriod>); 7] {
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
    pub fn validate_tariff_period(day: Weekday, rps: &[&RatePeriod]) -> Result<(), TariffError> {
        let hm = |val| Self::from_minute_of_day_unchecked(val);
        let mut cursor: u32 = 0;
        for rp in rps {
            Self::check_tier_order(&rp.rates)?;

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

    pub fn check_tier_order(rates: &[RateBlock]) -> Result<(), TariffError> {
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

    /// Check full year for tariff seasons
    ///
    /// This function will be used for mondtaory/optional tariff sutructures
    /// import is mondtaory , export is less
    /// The use of `empty_behavior` is to allow or reject this behavior
    ///
    /// # Errors
    ///
    /// returns [`SeasonError::Gap`] if there a day date not covered
    /// returns [`SeasonError::Overlap`] if there a day date covered multi times
    pub fn check_full_year(
        seasons: &[TariffSeason],
        empty_behavior: &EmptyBehavior,
    ) -> Result<(), SeasonError> {
        const ORIGIN: NaiveDate = match NaiveDate::from_ymd_opt(2020, 1, 1) {
            Some(d) => d,
            None => panic!("2017-01-01 is a valid date"),
        };

        const NUM_YEAR_DAY: usize = 366; // Feb 29 is priced as Feb 28

        if seasons.is_empty() {
            match empty_behavior {
                EmptyBehavior::Reject => return Err(SeasonError::Empty),
                EmptyBehavior::Allow => return Ok(()),
            }
        }

        let origin = ORIGIN;
        for day in origin.iter_days().take(NUM_YEAR_DAY) {
            let md = MonthDay::new_unchecked(day.month(), day.day());
            // WRONG for time-of-use tariffs
            let lookup_md = if md == MonthDay::new_unchecked(2, 29) {
                MonthDay::new_unchecked(2, 28) // Feb 29 prices as Feb 28
            } else {
                md
            };
            let count = seasons
                .iter()
                .filter(|s| s.season.contains(lookup_md))
                .count();
            if count == 0 {
                return Err(SeasonError::Gap(lookup_md));
            }
            if count > 1 {
                return Err(SeasonError::Overlap(lookup_md));
            }
        }

        Ok(())
    }
}
