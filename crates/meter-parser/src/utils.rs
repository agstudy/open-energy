use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Australia::Brisbane; // AEST
use crate::models::{ParserError};

fn parse_fuzzy_date(date_str: &str) -> Result<NaiveDate, ParserError> {
    // Try Standard NEM12 first
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%Y%m%d") {
        return Ok(d);
    }
    // Try D/MM/YYYY (your new example)
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%e/%m/%Y") {
        return Ok(d);
    }
    // Try DD/MM/YYYY
    if let Ok(d) = NaiveDate::parse_from_str(date_str, "%d/%m/%Y") {
        return Ok(d);
    }

    Err(ParserError::InvalidTimestamp)
}


pub fn parse_date(date_str: &str) -> Result<DateTime<Utc>, ParserError> {
    let naive_date = parse_fuzzy_date(date_str)?;

    let midnight = naive_date
        .and_hms_opt(0, 0, 0)
        .ok_or(ParserError::InvalidTimestamp)?;

    // Start of the day in AEST, then converted to UTC
    let utc_start = Brisbane
        .from_local_datetime(&midnight)
        .earliest()
        .ok_or(ParserError::InvalidTimestamp)?
        .with_timezone(&Utc);

    Ok(utc_start)
}


