use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Deserialize)]
struct HourMinuteRaw {
    hour: u32,
    minute: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(try_from = "HourMinuteRaw")]
pub struct HourMinute {
    hour: u32,
    minute: u32,
}

#[derive(Debug)]
pub struct InvalidHourMinute(u32, u32);

impl std::error::Error for InvalidHourMinute {}

impl TryFrom<HourMinuteRaw> for HourMinute {
    type Error = InvalidHourMinute;

    fn try_from(value: HourMinuteRaw) -> Result<Self, Self::Error> {
        HourMinute::new(value.hour, value.minute)
    }
}

impl std::fmt::Display for InvalidHourMinute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid time {:02}:{:02} (hour must be 0-23, minute 0-59)",
            self.0, self.1
        )
    }
}
#[derive(Debug)]
pub struct InvalidStrHourMinute(String);

impl std::fmt::Display for InvalidStrHourMinute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid time {} (hour must be 0-23, minute 0-59)",
            self.0
        )
    }
}

impl std::error::Error for InvalidStrHourMinute {}

impl TryFrom<&str> for HourMinute {
    type Error = InvalidStrHourMinute;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let err = || InvalidStrHourMinute(value.into());
        let (h, m) = value.split_once(":").ok_or_else(err)?;

        let hour = u32::from_str(h).map_err(|_| err())?;
        let minute = u32::from_str(m).map_err(|_| err())?;

        HourMinute::new(hour, minute).map_err(|_| InvalidStrHourMinute(value.into()))
    }
}

impl HourMinute {
    pub const MAX_MINUTE: u32 = 59;
    pub const MAX_HOUR: u32 = 23;

    pub fn new(hour: u32, minute: u32) -> Result<Self, InvalidHourMinute> {
        if (0..=Self::MAX_MINUTE).contains(&minute) && (0..=Self::MAX_HOUR).contains(&hour) {
            Ok(HourMinute { hour, minute })
        } else {
            Err(InvalidHourMinute(hour, minute))
        }
    }

    pub(crate) fn new_unchecked(hour: u32, minute: u32) -> Self {
        HourMinute { hour, minute }
    }
    /// Returns the previous minute, wrapping from 00:00 to 23:59.
    pub fn prev(self) -> HourMinute {
        if self.minute == 0 {
            HourMinute {
                hour: (self.hour + Self::MAX_HOUR) % 24,
                minute: Self::MAX_MINUTE,
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

    pub fn minute_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }

    pub fn from_minute_of_day(m: u32) -> Result<Self, InvalidHourMinute> {
        Self::new(m / 60, m % 60)
    }

    pub const fn min() -> Self {
        Self { hour: 0, minute: 0 }
    }

    pub const fn max() -> Self {
        Self {
            hour: Self::MAX_HOUR,
            minute: Self::MAX_MINUTE,
        }
    }
}

impl std::fmt::Display for HourMinute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl std::str::FromStr for HourMinute {
    type Err = InvalidStrHourMinute;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s)
    }
}

pub const START_OF_DAY: HourMinute = HourMinute::min();
pub const END_OF_DAY: HourMinute = HourMinute::max();

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
