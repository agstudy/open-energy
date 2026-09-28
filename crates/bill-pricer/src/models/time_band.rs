use bitflags::bitflags;
use serde::{Deserialize, Serialize};

use super::hour_minute::HourMinute;

bitflags! {
#[derive(Serialize, Deserialize, Debug,PartialEq, Clone, Copy)]
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

#[derive(Deserialize)]
pub struct TimeBandRow {
    pub start: HourMinute,
    pub end: HourMinute,
    pub days_of_week: Option<WeekDays>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(try_from = "TimeBandRow")]
pub struct TimeBand {
    start: HourMinute,
    end: HourMinute,
    days_of_week: Option<WeekDays>,
}

#[derive(Debug, thiserror::Error)]
pub struct InvalidTimeBand;

impl std::fmt::Display for InvalidTimeBand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TimeBand start must be <= end")
    }
}

impl TryFrom<TimeBandRow> for TimeBand {
    type Error = InvalidTimeBand;

    fn try_from(value: TimeBandRow) -> Result<Self, Self::Error> {
        TimeBand::new(value.start, value.end, value.days_of_week)
    }
}

impl TimeBand {
    pub fn new(
        start: HourMinute,
        end: HourMinute,
        days_of_week: Option<WeekDays>,
    ) -> Result<Self, InvalidTimeBand> {
        if start > end {
            Err(InvalidTimeBand)
        } else {
            Ok(TimeBand {
                start,
                end,
                days_of_week,
            })
        }
    }
    pub fn start(&self) -> HourMinute {
        self.start
    }
    pub fn end(&self) -> HourMinute {
        self.end
    }

    pub fn days_of_week(&self) -> Option<WeekDays> {
        self.days_of_week
    }
}
