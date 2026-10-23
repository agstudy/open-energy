use chrono::Weekday;
use domain::hour_minute::InvalidStrHourMinute;
use domain::{HourMinute, year_month::MonthDay};
use rust_decimal::Decimal;

use crate::InvalidTimeBand;

#[derive(Debug, thiserror::Error)]
pub enum SeasonError {
    #[error("seasons have gap at {0}")]
    Gap(MonthDay),
    #[error("seasons overlaps at {0}")]
    Overlap(MonthDay),
    #[error("no seasons defined at {0}")]
    NoSeasonForDate(MonthDay),
    #[error("empty season")]
    Empty,
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
    #[error(transparent)]
    Season(#[from] SeasonError),
}
