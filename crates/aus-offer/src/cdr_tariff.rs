use serde::Deserialize;

use crate::{cdr_demand::DemandTariff, cdr_domain::{RateDto, TimeOfUseDayTimeBand, TimeZone}};

/// Type of charge. Assumed to be `OTHER` if absent.
#[derive(Debug, Deserialize)]
pub enum TariffChargeType {
    #[serde(rename = "ENVIRONMENTAL")]
    Environmental,
    #[serde(rename = "REGULATED")]
    Regulated,
    #[serde(rename = "NETWORK")]
    Network,
    #[serde(rename = "METERING")]
    Metering,
    #[serde(rename = "RETAIL_SERVICE")]
    RetailService,
    #[serde(rename = "RCTI")]
    Rcti,
    #[serde(rename = "OTHER")]
    Other,
}

/// Specifies if daily supply charge is single or banded.
#[derive(Debug, Deserialize)]
pub enum DailySupplyChargeType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "BAND")]
    Band,
}

/// Specifies the type of rate applicable to this tariff period.
#[derive(Debug, Deserialize)]
pub enum RateBlockUType {
    #[serde(rename = "singleRate")]
    SingleRate,
    #[serde(rename = "timeOfUseRates")]
    TimeOfUseRates,
    #[serde(rename = "demandCharges")]
    DemandCharges,
}



/// EnergyPlanTariffPeriodV2SingleRate : Object representing a single rate. Required if _rateBlockUType_ is `singleRate`.
#[derive(Debug, Deserialize)]
pub struct SingleRateTariff {
    #[serde(rename = "rates")]
    pub rates: Vec<RateDto>,
}

/// The type of usage that the rate applies to.
#[derive(Debug, Deserialize)]
pub enum UsageType {
    #[serde(rename = "PEAK")]
    Peak,
    #[serde(rename = "OFF_PEAK")]
    OffPeak,
    #[serde(rename = "SHOULDER")]
    Shoulder,
    #[serde(rename = "SHOULDER1")]
    Shoulder1,
    #[serde(rename = "SHOULDER2")]
    Shoulder2,
}




#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeOfUseRatesTariff {
    pub rates: Vec<RateDto>,
    pub time_of_use: Vec<TimeOfUseDayTimeBand>,
    #[serde(rename = "type")]
    pub usage_type: UsageType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanTariff {
    /// Type of charge. Assumed to be `OTHER` if absent.
    #[serde(rename = "type")]
    pub charge_type: Option<TariffChargeType>,
    pub display_name: String,
    pub start_date: String,
    pub end_date: String,
    pub daily_supply_charge_type: Option<DailySupplyChargeType>,
    pub daily_supply_charge: Option<String>,
    pub time_zone: Option<TimeZone>,
    pub rate_block_u_type: RateBlockUType,
    pub time_of_use_rates: Option<Vec<TimeOfUseRatesTariff>>,
    pub demand_charges: Option<Vec<DemandTariff>>,
}
