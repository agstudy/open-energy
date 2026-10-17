use rust_decimal::Decimal;
use serde::Deserialize;

/// The type of the discount.
#[derive(Debug, Deserialize)]
pub enum DiscountType {
    #[serde(rename = "CONDITIONAL")]
    Conditional,
    #[serde(rename = "GUARANTEED")]
    Guaranteed,
    #[serde(rename = "OTHER")]
    Other,
}

/// The type of the discount. Mandatory if the discount _type_ is `CONDITIONAL`.
#[derive(Debug, Deserialize)]
pub enum DiscountCategory {
    #[serde(rename = "PAY_ON_TIME")]
    PayOnTime,
    #[serde(rename = "DIRECT_DEBIT")]
    DirectDebit,
    #[serde(rename = "GUARANTEED_DISCOUNT")]
    GuaranteedDiscount,
    #[serde(rename = "OTHER")]
    Other,
}

/// The method of calculation of the discount.
#[derive(Debug, Deserialize)]
pub enum DiscountCalculationType {
    #[serde(rename = "percentOfBill")]
    PercentOfBill,
    #[serde(rename = "percentOfUse")]
    PercentOfUse,
    #[serde(rename = "fixedAmount")]
    FixedAmount,
    #[serde(rename = "percentOverThreshold")]
    PercentOverThreshold,
}

#[derive(Debug, Deserialize)]
pub struct PercentOfUse {
    /// The rate of the discount applied to the _usageamount_.
    #[serde(rename = "rate")]
    pub rate: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct FixedAmount {
    /// The rate of the discount applied to the bill amount.
    #[serde(rename = "amount")]
    pub amount: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct PercentOfBill {
    /// The rate of the discount applied to the bill amount.
    #[serde(rename = "rate")]
    pub rate: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PercentOverThreshold {
    pub rate: Decimal,
    pub usage_amount: Decimal,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDiscount {
    /// The type of the discount.
    #[serde(rename = "type")]
    pub discount_type: DiscountType,
    pub category: Option<DiscountCategory>,
    pub method_u_type: DiscountCalculationType,
    pub percent_of_bill: Option<PercentOfBill>,
    pub percent_of_use: Option<PercentOfUse>,
    pub fixed_amount: Option<FixedAmount>,
    pub percent_over_threshold: Option<PercentOverThreshold>,
}
