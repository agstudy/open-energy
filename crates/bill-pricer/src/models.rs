use std::cmp::Ordering;

use bitflags::bitflags;
use rust_decimal::Decimal;

#[derive(Debug)]
pub struct HourMinute {
    pub hour: u32,
    pub minute: u32,
}

impl PartialEq for HourMinute {
    fn eq(&self, other: &Self) -> bool {
        self.hour == other.hour && self.minute == other.minute
    }
}

impl PartialOrd for HourMinute {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(
            self.hour
                .cmp(&other.hour)
                .then_with(|| self.minute.cmp(&other.minute)),
        )
    }
}

bitflags! {

    pub struct WeekDays: u8 {

        const MON = 0b0000001;
        const TUE = 0b0000010;
        const WED = 0b0000100;
        const THU = 0b0001000;
        const FRI = 0b0010000;
        const SAT = 0b0100000;
        const SUN = 0b1000000;
    }
}

pub struct TimeBand {
    pub start: HourMinute,
    pub end: HourMinute,
    pub days_of_week: Option<WeekDays>,
}

pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Option<Decimal>,
}

pub enum DiscountValueType {
    Percent,
    Absolute,
}

pub enum DiscountApplicationType {
    Total,
    Energy,
}

pub struct Discount {
    pub value_type: DiscountValueType,
    pub application_type: DiscountApplicationType,
    pub amount: Decimal,
}

pub struct RatePeriod {
    pub rates: Vec<RateBlock>,
    pub time_band: Option<TimeBand>,
}

impl RatePeriod {

    /// Returns true if this period covers `at`. `time_band: None` means
    /// "always applies" (used for flat/non TOU).
    /// Day-of-week matching not yet implemented.
    pub fn applies_at(&self, at: &HourMinute) -> bool {
        match &self.time_band {
            None => true,
            Some(tb) => tb.start <= *at && *at <= tb.end,
        }
    }
}

pub struct Tariff {
    pub import_tariff: Vec<RatePeriod>,
    pub export_tariff: Option<Vec<RatePeriod>>,
    pub discount: Option<Vec<Discount>>,
    pub supply_rate: Decimal,
}
