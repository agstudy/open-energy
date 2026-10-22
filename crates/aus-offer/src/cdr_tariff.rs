use bill_pricer::{
    tariff::{ConsumptionPeriod, TariffBuilder, TariffError},
    tariff_factory::TariffFactory,
};
use domain::year_month::MonthDay;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    cdr_demand::DemandTariff,
    cdr_domain::{
        BuildTimeBandError, ResetPeriod, SingleRates, TimeOfUseRates, TimeZone, all_starts,
        block_rates,
    },
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
#[derive(Debug, Deserialize, PartialEq, Clone, Eq, Hash)]
pub enum RateBlockUType {
    #[serde(rename = "singleRate")]
    SingleRates,
    #[serde(rename = "timeOfUseRates")]
    TimeOfUseRates,
    #[serde(rename = "demandCharges")]
    DemandCharges,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodTariff {
    /// Type of charge. Assumed to be `OTHER` if absent.
    #[serde(rename = "type")]
    pub charge_type: Option<TariffChargeType>,
    pub display_name: String,
    pub start_date: MonthDay,
    pub end_date: MonthDay,
    pub daily_supply_charge_type: Option<DailySupplyChargeType>,
    pub daily_supply_charge: Option<Decimal>,
    pub time_zone: Option<TimeZone>,
    pub rate_block_u_type: RateBlockUType,
    pub single_rate: Option<SingleRates>,
    pub time_of_use_rates: Option<Vec<TimeOfUseRates>>,
    pub demand_charges: Option<Vec<DemandTariff>>,
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

    #[error(transparent)]
    TimeBandError(#[from] BuildTimeBandError),
}

impl PeriodTariff {
    /// Builds a [`TariffBuilder`] for this period block by dispatching on
    /// [`RateBlockUType`].
    ///
    /// A `SingleRates` block is built via [`Self::single_rate_builder`];
    /// a `TimeOfUseRates` block via [`Self::time_of_use_builder`].
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError::MissingField`] if the field matching
    /// `rate_block_u_type` is absent (e.g. `singleRate` missing on a
    /// `SingleRates` block), and any error from the underlying builder
    /// (e.g. [`PeriodTariffError::Tariff`] from `TariffFactory`,
    /// [`PeriodTariffError::UnsupportedBandSupply`], or
    /// [`PeriodTariffError::TimeBandError`]).
    pub fn builder(&self) -> Result<TariffBuilder, PeriodTariffError> {
        match self.rate_block_u_type {
            RateBlockUType::SingleRates => self.single_rate_builder(),
            RateBlockUType::TimeOfUseRates => self.time_of_use_builder(),
            RateBlockUType::DemandCharges => Err(PeriodTariffError::NotImplemented(
                RateBlockUType::DemandCharges,
            )),
        }
    }
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

    /// Creates a builder for a single-rate tariff.
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError`] if the tariff's period definition is invalid
    /// or inconsistent with a single-rate structure.
    pub fn single_rate_builder(&self) -> Result<TariffBuilder, PeriodTariffError> {
        let single = self
            .single_rate
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("singleRate"))?;

        let rates = block_rates(&single.rates);

        let consumption_period = Self::get_consumption_period(&rates, single.reset_period.as_ref());

        let supply_rate = self.get_supply_rate()?;
        TariffFactory::multiflat(&rates, supply_rate, consumption_period)
            .map_err(PeriodTariffError::from)
    }

    /// Creates a builder for a time-of-use tariff.
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError`] if the tariff's time bands are invalid,
    /// overlapping, or leave gaps in coverage.
    pub fn time_of_use_builder(&self) -> Result<TariffBuilder, PeriodTariffError> {
        let tou_rates: Vec<&TimeOfUseRates> = self
            .time_of_use_rates
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("timeOfUseRates"))?
            .iter()
            .collect();

        let mut builder = TariffBuilder::default();
        let starts = all_starts(&tou_rates);
        for tou_period in tou_rates {
            let blocks = block_rates(&tou_period.rates);
            for time_band in &tou_period.time_of_use {
                for tb in time_band.to_time_band(&starts)? {
                    builder = builder.add_rate_period(&blocks, tb);
                }
            }
            builder = builder.consumption_period(Self::get_consumption_period(
                &blocks,
                tou_period.reset_period.as_ref(),
            ));
        }

        builder = builder.daily_supply(self.get_supply_rate()?);

        Ok(builder)
    }

    /// helper to parse consumption period
    fn get_consumption_period(
        rates: &[(Decimal, Decimal)],
        reset_period: Option<&ResetPeriod>,
    ) -> Option<ConsumptionPeriod> {
        if rates.len() > 1 {
            reset_period.as_ref().and_then(|p| match p {
                ResetPeriod::Daily => Some(ConsumptionPeriod::Day),
                ResetPeriod::Monthly => Some(ConsumptionPeriod::Month),
                ResetPeriod::BiMonthly => Some(ConsumptionPeriod::BiMonthly),
                ResetPeriod::Quarterly => Some(ConsumptionPeriod::Quarterly),
                _ => None,
            })
        } else {
            None
        }
    }
}
