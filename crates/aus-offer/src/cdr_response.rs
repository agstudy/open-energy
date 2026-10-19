use crate::{cdr_domain::GeographyDto, cdr_plan::PlanDetail};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PlanDetailResponse {
    pub data: PlanDetail,
}

#[derive(Debug, Deserialize)]
pub struct PlanResponseDto {
    pub data: PlansDataDto,
    pub meta: MetaDto,
    pub links: LinksDto,
}

#[derive(Debug, Deserialize)]
pub struct PlansDataDto {
    pub plans: Vec<PlanDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    #[serde(rename = "type")]
    pub plan_type: String,
    pub brand: String,
    pub plan_id: String,
    pub brand_name: String,
    pub display_name: String,
    pub last_updated: DateTime<Utc>,
    pub customer_type: String,
    pub effective_from: DateTime<Utc>,
    pub geography: Option<GeographyDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaDto {
    pub total_records: u32,
    pub total_pages: u32,
}

#[derive(Debug, Deserialize)]
pub struct LinksDto {
    #[serde(rename = "self")]
    pub self_: String,
    pub next: Option<String>,
    pub last: Option<String>,
}

impl std::fmt::Display for PlanResponseDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Plan Response")?;
        writeln!(f, "  Total pages:   {}", self.meta.total_pages)?;
        writeln!(f, "  Total records: {}", self.meta.total_records)?;
        for plan in &self.data.plans {
            writeln!(f, "  Brand name:   {}", plan.brand_name)?;
            writeln!(f, "  Plan id:   {}", plan.plan_id)?;
        }

        Ok(())
    }
}
