use bitflags::bitflags;
use rust_decimal::Decimal;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct HourMinute {
    hour: u32,
    minute: u32,
}

#[derive(Debug)]
pub struct InvalidHourMinute(u32, u32);

impl HourMinute {
    pub const MAX_MINUTE: u32 = 59;
    pub const MAX_HOUR: u32 = 23;

    pub fn new(hour: u32, minute: u32) -> Result<Self, InvalidHourMinute> {
        if (0..=59).contains(&minute) && (0..=23).contains(&hour) {
            Ok(HourMinute { hour, minute })
        } else {
            Err(InvalidHourMinute(hour, minute))
        }
    }
    pub fn prev(self) -> HourMinute {
        if self.minute == 0 {
            HourMinute {
                hour: (self.hour + 23) % 24,
                minute: 59,
            }
        } else {
            HourMinute {
                hour: self.hour,
                minute: self.minute - 1,
            }
        }
    }

    pub fn split_midnight(self, end: HourMinute) -> Vec<(HourMinute, HourMinute)> {
        if self >= end {
            vec![
                (
                    self,
                    Self {
                        hour: Self::MAX_HOUR,
                        minute: Self::MAX_MINUTE,
                    },
                ),
                (Self { hour: 0, minute: 0 }, end),
            ]
        } else {
            vec![(self, end)]
        }
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


    pub fn new (rate: Decimal, time_band: Option<TimeBand>) -> Self {
        RatePeriod {
            rates: vec![RateBlock {
                rate,
                lower_band: None,
            }],
            time_band,
        }
    }
}

pub struct Tariff {
    pub import_tariff: Vec<RatePeriod>,
    pub export_tariff: Option<Vec<RatePeriod>>,
    pub discount: Option<Vec<Discount>>,
    pub supply_rate: Decimal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prev_0hour_minute() {
        let hm = HourMinute {
            hour: 0,
            minute: 11,
        };
        assert_eq!(
            hm.prev(),
            HourMinute {
                hour: 0,
                minute: 10
            }
        );
    }

    #[test]
    fn test_prev_hour_0minute() {
        let hm = HourMinute { hour: 5, minute: 0 };
        assert_eq!(
            hm.prev(),
            HourMinute {
                hour: 4,
                minute: 59
            }
        );
    }
}
