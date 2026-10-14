use chrono::{DateTime, Utc};
use core::fmt;
use std::collections::HashMap;
use std::path::PathBuf;

use reqwest::{self, Client};
use serde::Deserialize;
use tokio::fs;
use tokio::task::{JoinError, JoinSet};

use crate::cdr_domain::GeographyDto;
use crate::cdr_plan::PlanDetail;
use crate::energy_map::ENERGY_BASE_URIS;
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
    #[error(transparent)]
    Io(#[from] std::io::Error),
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
    pub data: PlanDetail,
}

async fn save_plan(body: &str, plan_id: &str) {
    let path = format!("/tmp/plans/{plan_id}.json");
    fs::write(&path, body).await.unwrap();
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
) -> Result<Vec<PlanDto>, LoadError> {
    let mut url = format!(
        "{retailer_base_url}cds-au/v1/energy/plans?fuelType=ELECTRICITY&page-size={page_size}"
    );
    let mut all = Vec::new();
    loop {
        let body = fetch_url(client, url.as_str(), 1).await?;
        let result: PlanResponseDto =
            serde_json::from_str(body.as_str()).map_err(|e| LoadError::JsonInUrl {
                url: url.clone(),
                source: e,
            })?;
        all.extend(result.data.plans);
        match result.links.next {
            Some(next) => url = next,
            None => break,
        }
    }
    Ok(all)
}

pub async fn get_plan_detail(
    client: &Client,
    retailer_base_url: &str,
    plan_id: &str,
) -> Result<PlanDetail, LoadError> {
    let url = format!("{retailer_base_url}cds-au/v1/energy/plans/{plan_id}");
    let body = fetch_url(client, &url, 3).await?;

    save_plan(&body, plan_id).await;

    match serde_json::from_str::<PlanDetailResponse>(body.as_str()) {
        Err(e) => {
            let col = e.column();
            let snippet = &body[col.saturating_sub(100)..(col + 100).min(body.len())];
            eprintln!("parse error near, ..{snippet}...");
            Err(LoadError::JsonInUrl {
                url: url.clone(),
                source: e,
            })
        }
        Ok(v) => Ok(v.data),
    }
}

// let existing = list_files(&"/tmp/plans").await.unwrap();
// for plan in &body.data.plans {
//     let id_plan = &plan.plan_id;
//     if !existing.contains(id_plan) {
//         println!("Loading {}", id_plan);
//         get_plan_detail(&client, url, id_plan).await?;
//     }
// }

pub async fn get_all_plans(client: &Client) -> Result<HashMap<String, String>, LoadError> {
    let mut set: JoinSet<Result<Vec<(String, String)>, LoadError>> = JoinSet::new();

    for (_, &url) in ENERGY_BASE_URIS.iter() {
        // Arc handle for the task (refcount bump, not a Client clone)
        let client = client.clone();
        set.spawn(async move {
            let plans = get_retailer_plans(&client.clone(), url, 1000).await?;

            let result = plans
                .iter()
                .map(|p| (p.plan_id.clone(), url.to_string()))
                .collect::<Vec<(String, String)>>();
            Ok(result)
        });
    }
    let mut result: HashMap<String, String> = HashMap::new();
    while let Some(res) = set.join_next().await {
        for (plan_id, url) in res?? {
            result.insert(plan_id, url);
        }
    }

    Ok(result)
}
const CACHE_DIR: &str = "/tmp/plans";

pub async fn get_all_plans_details(
    client: &Client,
) -> Result<HashMap<String, PlanDetail>, LoadError> {
    let mut set: JoinSet<Result<PlanDetail, LoadError>> = JoinSet::new();
    let plans = get_all_plans(&client).await?;

    for (plan_id, url) in plans {
        let client = client.clone();
        set.spawn(async move {
            let path = PathBuf::from(CACHE_DIR).join(format!("{}.json", &plan_id));

            let plan = if tokio::fs::try_exists(&path).await? {
                let body = tokio::fs::read_to_string(&path).await?;
                let res = serde_json::from_str::<PlanDetailResponse>(&body)?;
                res.data
            } else {
                get_plan_detail(&client, &url, &plan_id).await?
            };
            Ok(plan)
        });
    }
    let mut result: HashMap<String, PlanDetail> = HashMap::new();
    while let Some(res) = set.join_next().await {
        let plan = res??;
        result.insert(plan.plan_id.clone(), plan);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vic_offer() {
        let json = include_str!("../data/ORI1145183MBE2@EME.json");
        serde_json::from_str::<PlanDetailResponse>(&json).unwrap();
    }
}
