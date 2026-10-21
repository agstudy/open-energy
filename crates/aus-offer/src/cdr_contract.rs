use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
};

use bill_pricer::tariff::{Tariff, TariffBuilder, TariffError};
use domain::hour_minute::{END_OF_DAY, START_OF_DAY};
use serde::Deserialize;

use crate::{
    cdr_discount::PlanDiscount,
    cdr_domain::{TariffUType, TimeZone, all_starts, block_rates},
    cdr_eligibility::PlanEligibility,
    cdr_solar::{PayerType, SolarScheme, SolarTariff},
    cdr_tariff::{PeriodTariff, PeriodTariffError},
};

/// The pricing model for the contract. Contracts for gas must use `SINGLE_RATE`. Note that the detail for the enumeration values are:<ul><li>`SINGLE_RATE`: all energy usage is charged at a single unit rate no matter when it is consumed. Multiple unit rates may exist that correspond to varying volumes of usage i.e. a 'block' or 'step' tariff (first 50kWh @ X cents, next 50kWh at Y cents etc.)</li><li>`SINGLE_RATE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`TIME_OF_USE`: energy usage is charged at unit rates that vary dependent on time of day and day of week that the energy is consumed</li><li>`TIME_OF_USE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`FLEXIBLE`: energy usage is charged at unit rates that vary based on external factors</li><li>`FLEXIBLE_CONT_LOAD`: as above, but with an additional, separate unit rate charged for all energy usage from a controlled load i.e. separately metered appliance like hot water service, pool pump etc.</li><li>`QUOTA`: all energy usage is charged at a single fixed rate, up to a specified usage quota/allowance. All excess usage beyond the allowance is then charged at a single unit rate. i.e. $50/month for up to 150kWh included usage.</li></ul>
#[derive(Debug, Deserialize, Clone, PartialEq, Eq, Hash)]
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
    pub solar_feed_in_tariff: Option<Vec<SolarTariff>>,
    pub tariff_period: Vec<PeriodTariff>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseContractError {
    #[error("tariff: {0}")]
    ParseTariff(#[from] PeriodTariffError),

    #[error("pricing model {0:?} is not implemented yet")]
    NotImplemented(PricingModel),

    #[error("solar multioptions is unsupported")]
    UnsupportedMultipleOptions,
}

pub struct PlanTariff {
    // each leg is a season
    pub import: Vec<Tariff>,
    // solar have multiple schema generally only one is used in real pricing (current or other)
    pub export: HashMap<(SolarScheme, PayerType), Tariff>,
}

impl PlanContract {
    /// Builds the list of tariffs described by this contract.
    ///
    /// # Errors
    ///
    /// Returns [`ParseContractError`] if the contract data is malformed, e.g.
    /// missing required fields or an unrecognised tariff structure.
    pub fn build_tariff(&self) -> Result<PlanTariff, ParseContractError> {
        if self.pricing_model.base() == BasePricingModel::Quota {
            return Err(ParseContractError::NotImplemented(PricingModel::Quota));
        }
        Ok(PlanTariff {
            import: self.build_import_tariffs()?,
            export: self.build_export_tariffs()?,
        })
    }

    pub fn build_import_tariffs(&self) -> Result<Vec<Tariff>, ParseContractError> {
        let builders = self
            .tariff_period
            .iter()
            .map(PeriodTariff::builder)
            .collect::<Result<Vec<_>, _>>()?;

        let tariffs = builders
            .into_iter()
            .map(TariffBuilder::build)
            .collect::<Result<Vec<_>, _>>()
            .map_err(PeriodTariffError::from)?;
        Ok(tariffs)
    }

    pub fn build_export_tariffs(
        &self,
    ) -> Result<HashMap<(SolarScheme, PayerType), Tariff>, ParseContractError> {
        let mut solar_map: HashMap<(SolarScheme, PayerType), Tariff> = HashMap::new();

        let mut by_key: HashMap<(SolarScheme, PayerType, TariffUType), Vec<&SolarTariff>> =
            HashMap::new();

        if let Some(solar_feed_in_tariff) = &self.solar_feed_in_tariff {
            for feed in solar_feed_in_tariff {
                if !feed.is_invalid() {
                    by_key
                        .entry((feed.scheme, feed.payer_type, feed.tariff_u_type))
                        .or_default()
                        .push(feed);
                }
            }

            for ((scheme, payer, tariff_type), feeds) in by_key {
                let mut builder = TariffBuilder::default();
                match tariff_type {
                    TariffUType::SingleTariff => {
                        /* all feeds here are SingleTariff */
                        Self::build_solar_single_tariff(&feeds, &mut builder)?;
                    }
                    TariffUType::TimeOfUseTariff => {
                        /* all feeds here are TimeOfUseTariff */

                        let starts = Self::get_all_starts(&feeds);
                        for feed in &feeds {
                            Self::build_solar_tou_tariff(feed, &mut builder, &starts)?;
                        }
                    }
                }

                match builder.build() {
                    Ok(t) => {
                        solar_map.insert((scheme, payer), t);
                    }
                    Err(TariffError::Overlap(..)) => {
                        return Err(ParseContractError::UnsupportedMultipleOptions);
                    }
                    Err(e) => return Err(PeriodTariffError::from(e).into()),
                }
            }
        }
        Ok(solar_map)
    }

    fn build_solar_tou_tariff(
        feed: &SolarTariff,
        builder: &mut TariffBuilder,
        starts: &HashSet<String>,
    ) -> Result<(), ParseContractError> {
        let tou_rates = feed
            .time_of_use_rates
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("timeVaryingTariffs"))?;

        for tou_period in tou_rates {
            let blocks = block_rates(&tou_period.rates);
            for time_band in &tou_period.time_of_use {
                for tb in time_band
                    .to_time_band(starts)
                    .map_err(PeriodTariffError::from)?
                {
                    builder.add_export_rate_period(&blocks, tb);
                }
            }
        }
        Ok(())
    }

    fn build_solar_single_tariff(
        feeds: &[&SolarTariff],
        builder: &mut TariffBuilder,
    ) -> Result<(), ParseContractError> {
        let mut rates = Vec::new();
        
        for feed in feeds {
            rates.extend(&feed.get_single()?.rates);
        }

        builder
            .export_rate_period(|s| s.rates(&block_rates(rates)).time_band(START_OF_DAY, END_OF_DAY, None))
            .map_err(PeriodTariffError::from)?;

        Ok(())
    }

    fn get_all_starts(feeds: &[&SolarTariff]) -> HashSet<String> {
        let mut tou_rates = Vec::new();
        for feed in feeds {
            if let Some(rates) = &feed.time_of_use_rates {
                tou_rates.extend(rates);
            }
        }

        all_starts(&tou_rates)
    }

    // create tariffs
}
