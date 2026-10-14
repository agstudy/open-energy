
use serde::{Deserialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanEligibility {
    /// The type of the eligibility restriction.<br/>The `CONTINGENT_PLAN` value indicates that the plan is contingent on the customer taking up an alternate fuel plan from the same retailer (for instance, if the _fuelType_ is `ELECTRICITY` then a `GAS` plan from the same retailer must be taken up).
    #[serde(rename = "type")]
    pub eligibility_type: EligibilityType,
    pub information: String,
}

/// The type of the eligibility restriction.<br/>The `CONTINGENT_PLAN` value indicates that the plan is contingent on the customer taking up an alternate fuel plan from the same retailer (for instance, if the _fuelType_ is `ELECTRICITY` then a `GAS` plan from the same retailer must be taken up).
#[derive(Debug, Deserialize)]
pub enum EligibilityType {
    #[serde(rename = "EXISTING_CUST")]
    ExistingCust,
    #[serde(rename = "EXISTING_POOL")]
    ExistingPool,
    #[serde(rename = "EXISTING_SOLAR")]
    ExistingSolar,
    #[serde(rename = "EXISTING_BATTERY")]
    ExistingBattery,
    #[serde(rename = "EXISTING_SMART_METER")]
    ExistingSmartMeter,
    #[serde(rename = "EXISTING_BASIC_METER")]
    ExistingBasicMeter,
    #[serde(rename = "SENIOR_CARD")]
    SeniorCard,
    #[serde(rename = "SMALL_BUSINESS")]
    SmallBusiness,
    #[serde(rename = "NO_SOLAR_FIT")]
    NoSolarFit,
    #[serde(rename = "NEW_CUSTOMER")]
    NewCustomer,
    #[serde(rename = "ONLINE_ONLY")]
    OnlineOnly,
    #[serde(rename = "REQ_EQUIP_SUPPLIER")]
    ReqEquipSupplier,
    #[serde(rename = "THIRD_PARTY_ONLY")]
    ThirdPartyOnly,
    #[serde(rename = "SPORT_CLUB_MEMBER")]
    SportClubMember,
    #[serde(rename = "ORG_MEMBER")]
    OrgMember,
    #[serde(rename = "SPECIFIC_LOCATION")]
    SpecificLocation,
    #[serde(rename = "MINIMUM_USAGE")]
    MinimumUsage,
    #[serde(rename = "LOYALTY_MEMBER")]
    LoyaltyMember,
    #[serde(rename = "GROUP_BUY_MEMBER")]
    GroupBuyMember,
    #[serde(rename = "CONTINGENT_PLAN")]
    ContingentPlan,
    #[serde(rename = "OTHER")]
    Other,
}

