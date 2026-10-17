use serde::Deserialize;

use crate::cdr_tariff::{SingleRates, TimeOfUseRates};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSolarTariff {
    pub scheme: SolarScheme,
    pub tariff_u_type: TariffUType,
    pub single_rates: Option<SingleRates>,
    #[serde(rename = "timeVaryingTariffs")]
    pub time_of_use_rates: Option<Vec<TimeOfUseRates>>,
}

/// The applicable scheme.
#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
pub enum TariffUType {
    #[serde(rename = "singleTariff")]
    SingleTariff,
    #[serde(rename = "timeVaryingTariffs")]
    TimeOfUseTariff,
}
