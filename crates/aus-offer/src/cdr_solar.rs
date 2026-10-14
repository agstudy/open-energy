

use serde::{Deserialize};

use crate::cdr_domain::{RateDto, TimeOfUseDayTimeBand};

#[derive( Debug, Deserialize)]
pub struct PlanSolarTariff {
    /// The applicable scheme.
    #[serde(rename = "scheme")]
    pub scheme: SolarScheme,
    /// Reference to the applicable tariff structure.
    #[serde(rename = "tariffUType")]
    pub tariff_u_type: TariffUType,
    #[serde(rename = "singleTariff")]
    pub single_tariff: Option<SolarSingleTariff>,
    /// Represents a tariff based on time of day. Mandatory if _tariffUType_ is set to `timeVaryingTariffs`.
    #[serde(rename = "timeVaryingTariffs")]
    pub time_varying_tariffs: Option<Vec<SolarTimeUseTariff>>
}

/// EnergyPlanSolarFeedInTariffV3SingleTariff : Represents a constant tariff. Mandatory if _tariffUType_ is set to `singleTariff`.
#[derive( Debug, Deserialize)]
pub struct SolarSingleTariff {
    /// Array of feed in rates.
    #[serde(rename = "rates")]
    pub rates: Vec<RateDto>,
    /// Usage period for which the block rate applies. Formatted according to [ISO 8601 Durations](https://en.wikipedia.org/wiki/ISO_8601#Durations) (excludes recurrence syntax). Defaults to `P1Y` if absent.
    #[serde(rename = "period", skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
}


/// The type of the charging time period. If absent applies to all periods.
#[derive( Debug, Deserialize)]
pub enum SolarChargingType {
    #[serde(rename = "PEAK")]
    Peak,
    #[serde(rename = "OFF_PEAK")]
    OffPeak,
    #[serde(rename = "SHOULDER")]
    Shoulder,
}


#[derive( Debug, Deserialize)]
pub struct SolarTimeUseTariff {
    /// The type of the charging time period. If absent applies to all periods.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub charging_type: Option<SolarChargingType>,
    /// Display name of the tariff.
    #[serde(rename = "displayName")]
    pub display_name: String,
    /// Array of feed in rates.
    #[serde(rename = "rates", skip_serializing_if = "Option::is_none")]
    pub rates: Option<Vec<RateDto>>,
    /// Usage period for which the block rate applies. Formatted according to [ISO 8601 Durations](https://en.wikipedia.org/wiki/ISO_8601#Durations) (excludes recurrence syntax). Defaults to `P1Y` if absent.
    #[serde(rename = "period", skip_serializing_if = "Option::is_none")]
    pub period: Option<String>,
    /// Array of time windows for which this tariff is applicable.
    #[serde(rename = "timeVariations")]
    pub time_of_use: Vec<TimeOfUseDayTimeBand>,

}


/// The applicable scheme.
#[derive( Debug, Deserialize)]
pub enum SolarScheme {
    #[serde(rename = "PREMIUM")]
    Premium,
    #[serde(rename = "CURRENT")]
    Current,
    #[serde(rename = "VARIABLE")]
    Variable,
    #[serde(rename = "OTHER")]
    Other,
}



/// Reference to the applicable tariff structure.
#[derive( Debug, Deserialize)]
pub enum TariffUType {
    #[serde(rename = "singleTariff")]
    SingleTariff,
    #[serde(rename = "timeVaryingTariffs")]
    TimeOfUseTariff,
}


