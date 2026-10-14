use chrono::{DateTime, Utc};
use core::fmt;

use reqwest::{self, Client};
use serde::Deserialize;
use std::sync::Arc;
use tokio::task::{JoinError, JoinSet};

use crate::{cdr_contract::PlanContract, energy_map::ENERGY_BASE_URIS};
// --errors

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("json parse failed for {url}: {source}")]
    JsonInUrl {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("join: {0}")]
    Join(#[from] JoinError),
    #[error("No base url for: {0}")]
    NoBaseUrl(String),
}

// ---- dto

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetailerDto {
    pub brand_name: String,
    pub public_base_uri: String,
    pub industries: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RetailersDto {
    #[serde(rename = "data")]
    pub retailers: Vec<RetailerDto>,
}

impl fmt::Display for RetailerDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.brand_name, self.public_base_uri)
    }
}

// -- dto plans

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

/// EnergyPlanGeography : Describes the geographical area that the plan is available for.
/// If absent then it is assumed the plan is not geographically limited.
#[derive(Clone, Default, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeographyDto {
    /// Array of valid Australian postcodes that are specifically excluded from the plan.
    /// Each element is a single four digit postcode (e.g., `3000`)
    /// or a range of postcodes defined by two four digit postcodes and a hyphen (e.g., `3000-3999`).
    pub excluded_postcodes: Option<Vec<String>>,
    /// Array of valid Australian postcodes that are included from the plan.
    /// If absent defaults to all non-excluded postcodes.
    /// Each element is a single four digit postcode (e.g., `3000`)
    /// or a range of postcodes defined by two four digit postcodes and a hyphen (e.g., `3000-3999`).
    pub included_postcodes: Option<Vec<String>>,
    /// Array of distributors for the plan. Must have at least one entry.
    pub distributors: Vec<String>,
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

impl fmt::Display for PlanResponseDto {
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

// -- plan detail

#[derive(Debug, Deserialize)]
pub struct PlanDetailResponse {
    pub data: PlanContract,
}








// -----requests

async fn fetch_url(client: &Client, url: &str, version: u32) -> Result<String, reqwest::Error> {
    let body = client
        .get(url)
        .header("x-v", version.to_string())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(body)
}

pub async fn get_retailers(client: &Client) -> Result<Vec<RetailerDto>, LoadError> {
    let url = "https://api.cdr.gov.au/cdr-register/v1/energy/data-holders/brands/summary";
    let body = fetch_url(client, url, 1).await?;
    let root: RetailersDto = serde_json::from_str(body.as_str())?;
    Ok(root.retailers)
}

pub async fn get_retailer_plans(
    client: &Client,
    retailer_base_url: &str,
    page_size: usize,
) -> Result<PlanResponseDto, LoadError> {
    let url = format!(
        "{retailer_base_url}cds-au/v1/energy/plans?fuelType=ELECTRICITY&page-size={page_size}"
    );

    let body = fetch_url(client, url.as_str(), 1).await?;
    let data: PlanResponseDto =
        serde_json::from_str(body.as_str()).map_err(|e| LoadError::JsonInUrl {
            url: url.clone(),
            source: e,
        })?;
    Ok(data)
}

pub async fn get_plan_detail(
    client: &Client,
    retailer_base_url: &str,
    plan_id: &str,
) -> Result<PlanDetailResponse, LoadError> {
    let url = format!("{retailer_base_url}cds-au/v1/energy/plans/{plan_id}");
    let body = fetch_url(client, &url, 3).await?;

    let data: PlanDetailResponse =
        serde_json::from_str(body.as_str()).map_err(|e| LoadError::JsonInUrl {
            url: url.clone(),
            source: e,
        })?;
    Ok(data)
}

pub async fn get_all_plans(
    client: Arc<reqwest::Client>,
) -> Result<Vec<(String, PlanResponseDto)>, LoadError> {
    let mut set = JoinSet::new();

    for (&brand, &url) in ENERGY_BASE_URIS.iter() {
        // Arc handle for the task (refcount bump, not a Client clone)
        let client = Arc::clone(&client);
        set.spawn(async move {
            let body = get_retailer_plans(&client, url, 1).await?;
            for plan in &body.data.plans {
                get_plan_detail(&client, url, &plan.plan_id).await?;
            }
            Ok::<(String, PlanResponseDto), LoadError>((brand.into(), body))
        });
    }
    let mut result = Vec::new();
    while let Some(res) = set.join_next().await {
        result.push(res??);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_retailers() {}
}
