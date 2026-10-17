use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::{cdr_contract::PlanContract, cdr_domain::GeographyDto};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDetail {
    pub plan_id: String,
    /// The date and time from which this plan is effective (i.e. is available for origination).
    /// Used to enable the articulation of products to the regime before they are available for customers to originate.
    pub effective_from: Option<DateTime<Utc>>,
    /// The date and time at which this plan will be retired and will no longer be offered.
    /// Used to enable the managed deprecation of plans.
    pub effective_to: Option<DateTime<Utc>>,
    pub display_name: Option<String>,
    /// The type of the plan.
    #[serde(rename = "type")]
    pub plan_type: PlanType,
    pub fuel_type: FuelType,
    pub brand: String,
    pub brand_name: String,
    pub customer_type: Option<CustomerType>,
    pub geography: Option<GeographyDto>,
    pub electricity_contract: PlanContract,
}

#[derive(Debug, Deserialize)]
pub enum PlanType {
    #[serde(rename = "STANDING")]
    Standing,
    #[serde(rename = "MARKET")]
    Market,
    #[serde(rename = "REGULATED")]
    Regulated,
}

#[derive(Debug, Deserialize)]
pub enum FuelType {
    #[serde(rename = "ELECTRICITY")]
    Electricity,
    #[serde(rename = "GAS")]
    Gas,
    #[serde(rename = "DUAL")]
    Dual,
}

#[derive(Debug, Deserialize)]
pub enum CustomerType {
    #[serde(rename = "RESIDENTIAL")]
    Residential,
    #[serde(rename = "BUSINESS")]
    Business,
}
