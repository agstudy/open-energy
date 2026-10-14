
use serde::Deserialize;





/// Specifies the charge specific time zone for calculation of the time of use thresholds.
/// If absent, timezone value in PlanContract is assumed.
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
///
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
    pub unit_price: String, // "0.2587"
    pub volume: Option<u32>, // 20000
    /// The measurement unit of rate. Assumed to be `KWH` if absent.
    pub measure_unit: Option<MeasureUnitEnum>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOfUseDayTimeBand {
    pub days: Vec<EnergyDaysEnum>,
    pub start_time: String,
    pub end_time: String,
}