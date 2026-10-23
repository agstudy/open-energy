use domain::year_month::SeasonRange;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::TimeBand;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Decimal,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountValueType {
    Percent,
    Absolute,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountApplicationType {
    Total,
    Energy,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Discount {
    pub value_type: DiscountValueType,
    pub application_type: DiscountApplicationType,
    pub amount: Decimal,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Copy, Clone)]
pub enum ConsumptionPeriod {
    Day,
    Month,
    BiMonthly,
    Quarterly,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RatePeriod {
    pub rates: Vec<RateBlock>,
    pub time_band: TimeBand,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TariffSeason {
    pub periods: Vec<RatePeriod>,
    pub season: SeasonRange,
}
