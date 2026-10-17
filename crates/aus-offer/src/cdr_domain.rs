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
