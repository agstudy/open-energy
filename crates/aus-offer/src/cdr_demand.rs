use rust_decimal::Decimal;
use serde::Deserialize;

use crate::cdr_domain::{EnergyDaysEnum, MeasureUnitEnum};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DemandTariff {
    pub amount: Decimal,
    pub measure_unit: Option<MeasureUnitEnum>,
    pub start_time: String,
    pub end_time: String,
    pub days: Option<Vec<EnergyDaysEnum>>,
    pub min_demand: Option<Decimal>,
    pub max_demand: Option<Decimal>,
    pub measurement_period: MeasurementPeriod,
    pub charge_period: ChargePeriod,
}

/// Application period for the demand tariff.
#[derive(Debug, Deserialize)]
pub enum MeasurementPeriod {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "MONTH")]
    Month,
    #[serde(rename = "TARIFF_PERIOD")]
    TariffPeriod,
}

/// Charge period for the demand tariff.
#[derive(Debug, Deserialize)]
pub enum ChargePeriod {
    #[serde(rename = "DAY")]
    Day,
    #[serde(rename = "MONTH")]
    Month,
    #[serde(rename = "TARIFF_PERIOD")]
    TariffPeriod,
}
