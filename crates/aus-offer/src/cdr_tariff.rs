use bill_pricer::tariff::{ConsumptionPeriod, TariffBuilder, TariffError};
use domain::{
    hour_minute::{END_OF_DAY, START_OF_DAY},
    year_month::{MonthDay, SeasonRange},
};
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
    pub fn build_season(&self, builder: &mut TariffBuilder) -> Result<(), PeriodTariffError> {
        match self.rate_block_u_type {
            RateBlockUType::SingleRates => self.build_season_single_rates(builder),
            RateBlockUType::TimeOfUseRates => self.build_season_tou_rates(builder),
            RateBlockUType::DemandCharges => Ok(()),
        }
    }
    pub fn get_supply_rate(&self) -> Result<Decimal, PeriodTariffError> {
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

    /// Creates single-rate season tariff rates
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError`] if the tariff's period definition is invalid
    /// or inconsistent with a single-rate structure.
    pub fn build_season_single_rates(
        &self,
        builder: &mut TariffBuilder,
    ) -> Result<(), PeriodTariffError> {
        let single = self.get_single()?;

        let rates: Vec<(Decimal, Decimal)> = block_rates(&single.rates);

        let season = SeasonRange {
            start_date: self.start_date,
            end_date: self.end_date,
        };
        builder
            .rate_period(season, |s| {
                s.rates(&rates).time_band(START_OF_DAY, END_OF_DAY, None)
            })
            .map_err(PeriodTariffError::from)?;

        Ok(())
    }

    /// Creates touseason tariff rates
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError`] if the tariff's period definition is invalid
    /// or inconsistent with a tou structure.
    pub fn build_season_tou_rates(
        &self,
        builder: &mut TariffBuilder,
    ) -> Result<(), PeriodTariffError> {
        let tou_rates: Vec<&TimeOfUseRates> = self
            .time_of_use_rates
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("timeOfUseRates"))?
            .iter()
            .collect();

        let starts = all_starts(&tou_rates);
        let season = SeasonRange {
            start_date: self.start_date,
            end_date: self.end_date,
        };
        for tou_period in tou_rates {
            let blocks = block_rates(&tou_period.rates);
            for time_band in &tou_period.time_of_use {
                for tb in time_band.to_time_band(&starts)? {
                    builder.add_rate_period(season, &blocks, tb);
                }
            }
        }
        Ok(())
    }

    /// Returns the single-rate payload for this feed-in entry.
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError::MissingField`] if `singleTariff` is absent.
    pub fn get_single(&self) -> Result<&SingleRates, PeriodTariffError> {
        self.single_rate
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("singleTariff"))
    }
}
/// helper to parse consumption period
#[must_use]
pub fn get_consumption_period(reset_period: Option<ResetPeriod>) -> Option<ConsumptionPeriod> {
    reset_period.as_ref().and_then(|p| match p {
        ResetPeriod::Daily => Some(ConsumptionPeriod::Day),
        ResetPeriod::Monthly => Some(ConsumptionPeriod::Month),
        ResetPeriod::BiMonthly => Some(ConsumptionPeriod::BiMonthly),
        ResetPeriod::Quarterly => Some(ConsumptionPeriod::Quarterly),
        _ => None,
    })
}
