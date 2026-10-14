
use serde::Deserialize;

use crate::{cdr_discount::PlanDiscount, cdr_domain::TimeZone, cdr_eligibility::PlanEligibility, cdr_solar::PlanSolarTariff, cdr_tariff::PlanTariff};





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
    pub tariff_period: Vec<PlanTariff>,
}
