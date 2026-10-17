use bill_pricer::{
    tariff::{ConsumptionPeriod, TariffBuilder, TariffError},
    tariff_factory::TariffFactory,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    cdr_demand::DemandTariff,
    cdr_domain::{DaysTimeBand, RateDto, TimeZone},
};

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
#[derive(Debug, PartialEq, Deserialize)]
pub enum DailySupplyChargeType {
    #[serde(rename = "SINGLE")]
    Single,
    #[serde(rename = "BAND")]
    Band,
}

/// Specifies the type of rate applicable to this tariff period.
#[derive(Debug, Deserialize, PartialEq)]
pub enum RateBlockUType {
    #[serde(rename = "singleRate")]
    SingleRates,
    #[serde(rename = "timeOfUseRates")]
    TimeOfUseRates,
    #[serde(rename = "demandCharges")]
    DemandCharges,
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
    #[serde(rename = "timeVariations")]
    pub time_of_use: Vec<DaysTimeBand>,
    #[serde(rename = "period")]
    pub reset_period: Option<ResetPeriod>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodTariff {
    /// Type of charge. Assumed to be `OTHER` if absent.
    #[serde(rename = "type")]
    pub charge_type: Option<TariffChargeType>,
    pub display_name: String,
    pub start_date: String,
    pub end_date: String,
    pub daily_supply_charge_type: Option<DailySupplyChargeType>,
    pub daily_supply_charge: Option<Decimal>,
    pub time_zone: Option<TimeZone>,
    pub rate_block_u_type: RateBlockUType,
    pub single_rate: Option<SingleRates>,
    pub time_of_use_rates: Option<Vec<TimeOfUseRates>>,
    pub demand_charges: Option<Vec<DemandTariff>>,
}

/// --- solar

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

#[derive(Debug, thiserror::Error)]
pub enum PeriodTariffError {
    #[error("tariff: {0}")]
    Tariff(#[from] TariffError),

    #[error("Missing field : {0}")]
    MissingField(&'static str),

    #[error("no implemented tariff block type: {0:?}")]
    NotImplemented(RateBlockUType),

    #[error("unsupported band supply")]
    UnsupportedBandSupply,
}

impl PeriodTariff {
    fn get_supply_rate(&self) -> Result<Decimal, PeriodTariffError> {
        if self
            .daily_supply_charge_type
            .as_ref()
            .unwrap_or(&DailySupplyChargeType::Single)
            == &DailySupplyChargeType::Band
        {
            return Err(PeriodTariffError::UnsupportedBandSupply);
        }
        self.daily_supply_charge
            .ok_or(PeriodTariffError::MissingField("dailySupplyCharge"))
    }

    pub fn build_single_rate(&self) -> Result<TariffBuilder, PeriodTariffError> {

        if self.rate_block_u_type == RateBlockUType::DemandCharges 
        {
            return self.build_demand();
        }
        let single = self
            .single_rate
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("singleRate"))?;

        let mut blocks = Vec::new();
        let mut prev_upper = Decimal::ZERO;
        for rate in &single.rates {
            blocks.push((rate.unit_price, prev_upper));
            if let Some(v) = rate.volume {
                prev_upper = v;
            }
        }

        let consumption_period = if blocks.len() > 1 {
            single.reset_period.as_ref().and_then(|p| match p {
                ResetPeriod::Daily => Some(ConsumptionPeriod::Day),
                ResetPeriod::Monthly => Some(ConsumptionPeriod::Month),
                ResetPeriod::BiMonthly => Some(ConsumptionPeriod::BiMonthly),
                ResetPeriod::Quarterly => Some(ConsumptionPeriod::Quarterly),
                _ => None,
            })
        } else {
            None
        };

        let supply_rate = self.get_supply_rate()?;
        TariffFactory::multiflat(&blocks, supply_rate, consumption_period)
            .map_err(PeriodTariffError::from)
    }

    pub fn build_demand(&self) -> Result<TariffBuilder, PeriodTariffError> {
        Err(PeriodTariffError::NotImplemented(
            RateBlockUType::DemandCharges,
        ))
    }

    pub fn build_time_of_use(&self) -> Result<TariffBuilder, PeriodTariffError> {
        Err(PeriodTariffError::NotImplemented(
            RateBlockUType::TimeOfUseRates,
        ))
    }
}
