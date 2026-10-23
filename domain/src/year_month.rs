use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MonthDay {
    month: u32,
    day: u32,
}

impl From<MonthDay> for String {
    fn from(value: MonthDay) -> Self {
        format!("{:02}-{:02}", value.month, value.day)
    }
}

impl MonthDay {
    pub const MAX_DAY: u32 = 31;
    pub const MAX_MONTH: u32 = 12;
    // Builds an year and month pair.
    ///
    /// # Errors
    ///
    /// Returns [`YearMonthError`]
    pub fn new(month: u32, day: u32) -> Result<Self, MonthDayError> {
        if (1..=Self::MAX_DAY).contains(&day) && (1..=Self::MAX_MONTH).contains(&month) {
            Ok(MonthDay { month, day })
        } else {
            Err(MonthDayError::MonthDay(month, day))
        }
    }

    #[must_use]
    pub fn new_unchecked(month: u32, day: u32) -> Self {
        MonthDay { month, day }
    }

    #[must_use]
    pub fn month(&self) -> u32 {
        self.month
    }

    #[must_use]
    pub fn day(&self) -> u32 {
        self.day
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MonthDayError {
    #[error("invalid season {0:02}-{1:02} (month must be 1-12, day 1-31)")]
    MonthDay(u32, u32),
    #[error("invalid season {0} (month must be 1-12, day 1-31)")]
    MonthDayStr(String),
}

impl TryFrom<&str> for MonthDay {
    type Error = MonthDayError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let err = || MonthDayError::MonthDayStr(value.into());
        let (y, m) = value.split_once('-').ok_or_else(err)?;

        let month = u32::from_str(y).map_err(|_| err())?;
        let day = u32::from_str(m).map_err(|_| err())?;

        MonthDay::new(month, day)
    }
}

impl TryFrom<String> for MonthDay {
    type Error = MonthDayError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.as_str().try_into()
    }
}

impl std::fmt::Display for MonthDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}-{:02}", self.month, self.day)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Copy)]
pub struct SeasonRange {
    pub start_date: MonthDay,
    pub end_date: MonthDay,
}

impl SeasonRange {
    #[must_use]
    pub fn contains(&self, md: MonthDay) -> bool {
        if self.start_date <= self.end_date {
            md >= self.start_date && md <= self.end_date
        } else {
            md >= self.start_date || md <= self.end_date
        }
    }
    #[must_use]
    pub fn full_year() -> Self {
        Self {
            start_date: MonthDay::new_unchecked(1, 1),
            end_date: MonthDay::new_unchecked(12, 31),
        }
    }
}
#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    pub fn test_happy_monthday() {
        assert_eq!(
            <&str as TryInto<MonthDay>>::try_into("01-29").unwrap(),
            "1-29".try_into().unwrap()
        );
        assert_eq!(MonthDay::new(1, 29).unwrap(), "1-29".try_into().unwrap());
        assert_eq!(
            MonthDay::new(1, 29).unwrap(),
            MonthDay::new_unchecked(1, 29)
        );
    }

    #[test]
    pub fn test_bad_monthday() {
        assert!(matches!(
            <&str as TryInto<MonthDay>>::try_into("29-1"),
            Err(MonthDayError::MonthDay(_, _))
        ));

        assert!(matches!(
            <&str as TryInto<MonthDay>>::try_into("13-01"),
            Err(MonthDayError::MonthDay(_, _))
        ));

        assert!(matches!(
            <&str as TryInto<MonthDay>>::try_into("00-10"),
            Err(MonthDayError::MonthDay(_, _))
        ));

        assert!(matches!(
            <&str as TryInto<MonthDay>>::try_into("0101"),
            Err(MonthDayError::MonthDayStr(_))
        ));
    }

    #[test]
    pub fn test_happy_season_range() {
        let sr = SeasonRange {
            start_date: "11-01".try_into().unwrap(),
            end_date: "03-31".try_into().unwrap(),
        };
        assert!(
            sr.contains("12-15".try_into().unwrap()),
            "12-15 is inside 11-01/03-31"
        );
        assert!(
            sr.contains("02-10".try_into().unwrap()),
            "02-10 is inside 11-01/03-31"
        );
        assert!(
            !sr.contains("07-01".try_into().unwrap()),
            "07-01 is outside 11-01/03-31"
        );
    }

    #[test]
    fn month_day_ord_is_calendar() {
        assert!(MonthDay::new_unchecked(1, 1) < MonthDay::new_unchecked(12, 31));
        assert!(MonthDay::new_unchecked(2, 28) < MonthDay::new_unchecked(3, 1));
        assert!(MonthDay::new_unchecked(2, 29) < MonthDay::new_unchecked(3, 1));
    }

    #[test]
    fn test_wrapping_season_contains() {
        let winter = SeasonRange {
            start_date: "10-01".try_into().unwrap(),
            end_date: "03-31".try_into().unwrap(),
        };
        assert!(winter.contains("01-15".try_into().unwrap())); // Jan: inside (wrap)
        assert!(winter.contains("11-15".try_into().unwrap())); // Nov: inside
        assert!(!winter.contains("06-15".try_into().unwrap())); // Jun: outside
    }
}
