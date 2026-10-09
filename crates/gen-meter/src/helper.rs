use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

use chrono_tz::Tz;

#[derive(Debug, thiserror::Error)]
pub enum LocalConversionError {
    #[error("invalid timezone {tz:?}")]
    InvalidTimezone {
        tz: String,
        #[source]
        source: chrono_tz::ParseError,
    },
    #[error("ambiguous or nonexistent local time {local} in {tz}")]
    AmbiguousLocalTime { local: NaiveDateTime, tz: String },
    #[error("failed to parse datetime")]
    Parse(#[from] chrono::ParseError),
}

fn parse_naive_datetime(s: &str) -> Result<NaiveDateTime, chrono::ParseError> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
}

fn utc_from_local(
    local: NaiveDateTime,
    tz_str: &str,
) -> Result<DateTime<Utc>, LocalConversionError> {
    let tz: Tz = tz_str
        .parse()
        .map_err(|source| LocalConversionError::InvalidTimezone {
            tz: tz_str.into(),
            source,
        })?;

    tz.from_local_datetime(&local)
        .single()
        .ok_or_else(|| LocalConversionError::AmbiguousLocalTime {
            local,
            tz: tz_str.into(),
        })
        .map(|dt| dt.with_timezone(&Utc))
}

pub(crate) fn utc_from_local_str(
    s: &str,
    tz_str: &str,
) -> Result<DateTime<Utc>, LocalConversionError> {
    let naive = parse_naive_datetime(s)?;
    utc_from_local(naive, tz_str)
}
