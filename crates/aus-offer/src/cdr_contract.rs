use bill_pricer::tariff::{Tariff, TariffBuilder};
use serde::Deserialize;

use crate::{
    cdr_discount::PlanDiscount,
    cdr_domain::TimeZone,
    cdr_eligibility::PlanEligibility,
    cdr_solar::PlanSolarTariff,
    cdr_tariff::{PeriodTariff, PeriodTariffError},
};

/// The pricing model for the contract. Contracts for gas must use `SINGLE_RATE`. Note that the detail for the enumeration values are:<ul><li>`SINGLE_RATE`: all energy usage is charged at a single unit rate no matter when it is consumed. Multiple unit rates may exist that correspond to varying volumes of usage i.e. a 'block' or 'step' tariff (first 50kWh @ X cents, next 50kWh at Y cents etc.)</li><li>`SINGLE_RATE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`TIME_OF_USE`: energy usage is charged at unit rates that vary dependent on time of day and day of week that the energy is consumed</li><li>`TIME_OF_USE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`FLEXIBLE`: energy usage is charged at unit rates that vary based on external factors</li><li>`FLEXIBLE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`QUOTA`: all energy usage is charged at a single fixed rate, up to a specified usage quota/allowance. All excess usage beyond the allowance is then charged at a single unit rate. i.e. $50/month for up to 150kWh included usage.</li></ul>
#[derive(Debug, Deserialize)]
pub enum PricingModel {
    #[serde(rename = "SINGLE_RATE")]
    SingleRate,
    #[serde(rename = "SINGLE_RATE_CONT_LOAD")]
    SingleRateContLoad,
    #[serde(rename = "TIME_OF_USE")]
    TimeOfUse,
    #[serde(rename = "TIME_OF_USE_CONT_LOAD")]
    TimeOfUseContLoad,
    #[serde(rename = "FLEXIBLE")]
    Flexible,
    #[serde(rename = "FLEXIBLE_CONT_LOAD")]
    FlexibleContLoad,
    #[serde(rename = "QUOTA")]
    Quota,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasePricingModel {
    SingleRate,
    TimeOfUse,
    Flexible,
    Quota,
}

impl PricingModel {
    #[must_use]
    pub fn base(&self) -> BasePricingModel {
        match self {
            PricingModel::SingleRate | PricingModel::SingleRateContLoad => {
                BasePricingModel::SingleRate
            }
            PricingModel::TimeOfUse | PricingModel::TimeOfUseContLoad => {
                BasePricingModel::TimeOfUse
            }
            PricingModel::Flexible | PricingModel::FlexibleContLoad => BasePricingModel::Flexible,
            PricingModel::Quota => BasePricingModel::Quota,
        }
    }
    #[must_use]
    pub fn has_controlled_load(&self) -> bool {
        matches!(
            self,
            PricingModel::SingleRateContLoad
                | PricingModel::TimeOfUseContLoad
                | PricingModel::FlexibleContLoad
        )
    }
}

/// The type of the incentive.
#[derive(Debug, Deserialize)]
pub enum IncentiveCategory {
    #[serde(rename = "GIFT")]
    Gift,
    #[serde(rename = "ACCOUNT_CREDIT")]
    AccountCredit,
    #[serde(rename = "OTHER")]
    Other,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanIncentive {
    pub category: IncentiveCategory,
    pub eligibility: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanContract {
    pub pricing_model: PricingModel,
    pub time_zone: Option<TimeZone>,
    pub incentives: Option<Vec<PlanIncentive>>,
    pub discounts: Option<Vec<PlanDiscount>>,
    pub eligibility: Option<Vec<PlanEligibility>>,
    pub solar_feed_in_tariff: Option<Vec<PlanSolarTariff>>,
    pub tariff_period: Vec<PeriodTariff>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseContractError {
    #[error("tariff: {0}")]
    ParseTariff(#[from] PeriodTariffError),

    #[error("pricing model {0:?} is not implemented yet")]
    NotImplemented(PricingModel),
}

impl PlanContract {
    /// Builds the list of tariffs described by this contract.
    ///
    /// # Errors
    ///
    /// Returns [`ParseContractError`] if the contract data is malformed, e.g.
    /// missing required fields or an unrecognised tariff structure.
    pub fn build_tariff(&self) -> Result<Vec<Tariff>, ParseContractError> {
        if self.pricing_model.base() == BasePricingModel::Quota {
            return Err(ParseContractError::NotImplemented(PricingModel::Quota));
        }

        let builders = self
            .tariff_period
            .iter()
            .map(PeriodTariff::builder)
            .collect::<Result<Vec<_>, _>>()?;

        // add cload , solar ,fees  .. here

        // create tariffs
        let tariffs = builders
            .into_iter()
            .map(TariffBuilder::build)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PeriodTariffError::from)?;
        Ok(tariffs)
    }
}
