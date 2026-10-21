use std::collections::HashSet;

use bill_pricer::{InvalidTimeBand, TimeBand, WeekDays};
use domain::{
    HourMinute,
    hour_minute::{InvalidStrHourMinute, START_OF_DAY},
};
use rust_decimal::Decimal;
use serde::Deserialize;

/// Specifies the charge specific time zone for calculation of the time of use thresholds.
/// If absent, timezone value in `PlanContract` is assumed.
#[derive(Debug, Deserialize)]
pub enum TimeZone {
    #[serde(rename = "LOCAL")]
    Local,
    #[serde(rename = "AEST")]
    Aest,
}

#[derive(Debug, Deserialize)]
pub enum MeasureUnitEnum {
    #[serde(rename = "KWH")]
    Kwh,
    #[serde(rename = "KVA")]
    Kva,
    #[serde(rename = "KVAR")]
    Kvar,
    #[serde(rename = "KVARH")]
    Kvarh,
    #[serde(rename = "KW")]
    Kw,
    #[serde(rename = "DAYS")]
    Days,
    #[serde(rename = "METER")]
    Meter,
    #[serde(rename = "MONTH")]
    Month,
}

#[derive(Debug, Deserialize)]
pub enum EnergyDaysEnum {
    #[serde(rename = "SUN")]
    Sun,
    #[serde(rename = "MON")]
    Mon,
    #[serde(rename = "TUE")]
    Tue,
    #[serde(rename = "WED")]
    Wed,
    #[serde(rename = "THU")]
    Thu,
    #[serde(rename = "FRI")]
    Fri,
    #[serde(rename = "SAT")]
    Sat,
    #[serde(rename = "PUBLIC_HOLIDAYS")]
    PublicHolidays,
}

#[derive(Debug, thiserror::Error)]
#[error("PUBLIC_HOLIDAYS is not a valid tariff day")]
pub struct NoHolidays;

impl EnergyDaysEnum {
    /// Converts this value into the corresponding weekday.
    ///
    /// # Errors
    ///
    /// Returns [`NoHolidays`] if the date falls on a public holiday and therefore
    /// has no weekday tariff classification.
    pub fn to_weekday(&self) -> Result<WeekDays, NoHolidays> {
        Ok(match self {
            EnergyDaysEnum::Sun => WeekDays::SUN,
            EnergyDaysEnum::Mon => WeekDays::MON,
            EnergyDaysEnum::Tue => WeekDays::TUE,
            EnergyDaysEnum::Wed => WeekDays::WED,
            EnergyDaysEnum::Thu => WeekDays::THU,
            EnergyDaysEnum::Fri => WeekDays::FRI,
            EnergyDaysEnum::Sat => WeekDays::SAT,
            EnergyDaysEnum::PublicHolidays => return Err(NoHolidays),
        })
    }
}

fn days_to_weekdays(days: &[EnergyDaysEnum]) -> Result<WeekDays, NoHolidays> {
    days.iter()
        .try_fold(WeekDays::empty(), |acc, d| Ok(acc | d.to_weekday()?))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateDto {
    pub unit_price: Decimal,     // "0.2587"
    pub volume: Option<Decimal>, // 20000
    /// The measurement unit of rate. Assumed to be `KWH` if absent.
    pub measure_unit: Option<MeasureUnitEnum>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DaysTimeBand {
    pub days: Vec<EnergyDaysEnum>,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildTimeBandError {
    #[error("invalid time: {0}")]
    InvalidStringTime(#[from] InvalidStrHourMinute),

    #[error(transparent)]
    InvalidTimeBand(#[from] InvalidTimeBand),

    #[error("Invalid TimeBand:")]
    Holidays(#[from] NoHolidays),
}

impl DaysTimeBand {
    /// Converts this DTO into one or more [`TimeBand`]s.
    ///
    /// A single band can yield more than one `TimeBand` when it crosses
    /// midnight (e.g. `21:00`–`03:00` splits into `21:00`–`23:59` and
    /// `00:00`–`02:59`).
    ///
    /// `endTime` on a `:00` minute is treated as half-open and normalised
    /// to the previous minute (e.g. `21:00` → `20:59`), so the resulting
    /// bands do not overlap at the boundary.
    ///
    /// # Errors
    ///
    /// - [`BuildTimeBandError::InvalidStringTime`] if `start_time` or
    ///   `end_time` is not a valid `HH:MM`.
    /// - [`BuildTimeBandError::NoDays`] if `days` is empty.
    /// - [`BuildTimeBandError::Holidays`] if `days` contains
    ///   `PUBLIC_HOLIDAYS`, which is not yet supported.
    /// - [`BuildTimeBandError::InvalidTimeBand`] if `start` is after `end`
    ///   after normalisation, or the band is otherwise rejected by
    ///   [`TimeBand::new`].
    pub fn to_time_band(
        &self,
        all_starts: &HashSet<String>,
    ) -> Result<Vec<TimeBand>, BuildTimeBandError> {
        let start: HourMinute = self.start_time.as_str().try_into()?;
        let mut end: HourMinute = self.end_time.as_str().try_into()?;
        if (end == START_OF_DAY && start != START_OF_DAY) || all_starts.contains(&self.end_time) {
            end = end.prev();
        }
        let days_of_week = Some(days_to_weekdays(&self.days)?);
        start
            .split_midnight(end)
            .into_iter()
            .map(|(s, e)| Ok(TimeBand::new(s, e, days_of_week)?))
            .collect::<Result<Vec<_>, _>>()
    }
}

/// `EnergyPlanGeography` : Describes the geographical area that the plan is available for.
/// If absent then it is assumed the plan is not geographically limited.
#[derive(Clone, Default, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeographyDto {
    /// Array of valid Australian postcodes that are specifically excluded from the plan.
    /// Each element is a single four digit postcode (e.g., `3000`)
    /// or a range of postcodes defined by two four digit postcodes and a hyphen (e.g., `3000-3999`).
    pub excluded_postcodes: Option<Vec<String>>,
    /// Array of valid Australian postcodes that are included from the plan.
    /// If absent defaults to all non-excluded postcodes.
    /// Each element is a single four digit postcode (e.g., `3000`)
    /// or a range of postcodes defined by two four digit postcodes and a hyphen (e.g., `3000-3999`).
    pub included_postcodes: Option<Vec<String>>,
    /// Array of distributors for the plan. Must have at least one entry.
    pub distributors: Vec<String>,
}

/// `SingleRates` : Object representing a flat/multiflat(with reset period)
#[derive(Debug, Deserialize)]
pub struct SingleRates {
    #[serde(rename = "rates")]
    pub rates: Vec<RateDto>,
    #[serde(rename = "period")]
    pub reset_period: Option<ResetPeriod>,
}

/// The type of usage that the rate applies to.
#[derive(Debug, Deserialize)]
pub enum ResetPeriod {
    #[serde(rename = "P1D", alias = "day")]
    Daily,

    #[serde(rename = "P1M")]
    Monthly,

    #[serde(rename = "P2M")]
    BiMonthly,

    #[serde(rename = "P3M")]
    Quarterly,

    #[serde(rename = "P6M")]
    HalfYearly,

    #[serde(rename = "P1Y")]
    Yearly,

    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOfUseRates {
    pub rates: Vec<RateDto>,
    #[serde(alias = "timeVariations")]
    pub time_of_use: Vec<DaysTimeBand>,
    #[serde(rename = "period")]
    pub reset_period: Option<ResetPeriod>,
}

/// Reference to the applicable tariff structure.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TariffUType {
    #[serde(rename = "singleTariff")]
    SingleTariff,
    #[serde(rename = "timeVaryingTariffs")]
    TimeOfUseTariff,
}

/// Converts CDR `(rate, volume)` blocks into blocks with a lower limit.
///
/// `volume` is treated as a cumulative upper limit, so each block's lower
/// limit is the previous block's `volume` (0 for the first).
///
/// `[(r1, v), (r2, None)]` becomes `[(r1, 0), (r2, v)]`.
///
/// Accepts anything that yields `&RateDto`: `&Vec`, slices, `Vec<&RateDto>`,
/// or an iterator chain.
#[must_use]
pub fn block_rates<'a>(rates: impl IntoIterator<Item = &'a RateDto>) -> Vec<(Decimal, Decimal)> {
    let mut blocks = Vec::new();
    let mut prev_upper = Decimal::ZERO;
    for rate in rates {
        blocks.push((rate.unit_price, prev_upper));
        prev_upper = rate.volume.unwrap_or(Decimal::ZERO);
    }
    blocks
}

#[must_use]
pub fn all_starts(bands: &[&TimeOfUseRates]) -> HashSet<String> {
    let mut res = HashSet::new();
    for tou in bands {
        for x in &tou.time_of_use {
            res.insert(x.start_time.clone());
        }
    }
    res
}
