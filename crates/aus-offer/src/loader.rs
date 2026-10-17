use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bill_pricer::tariff::Tariff;
use reqwest::{self, Client};
use tokio::fs;
use tokio::task::{JoinError, JoinSet};

use crate::cdr_contract::ParseContractError;
use crate::cdr_plan::PlanDetail;
use crate::cdr_response::{PlanDetailResponse, PlanDto, PlanResponseDto};
use crate::energy_map::ENERGY_BASE_URIS;

const CACHE_DIR: &str = "/tmp/plans";

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
    #[error(transparent)]
    ParseError(#[from] ParseContractError),
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

pub async fn get_all_retailer_plans(client: &Client) -> Result<HashMap<String, String>, LoadError> {
    let mut set: JoinSet<Result<Vec<(String, String)>, LoadError>> = JoinSet::new();

    for &url in ENERGY_BASE_URIS.values() {
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

pub fn parse_offer_file(path: &Path) -> Result<Vec<Tariff>, LoadError> {
    let body = std::fs::read_to_string(path)?;
    let res = serde_json::from_str::<PlanDetailResponse>(&body)?;
    let tariff = res.data.electricity_contract.build_tariff()?;
    Ok(tariff)
}

pub async fn get_all_plans_details(
    client: &Client,
) -> Result<HashMap<String, PlanDetail>, LoadError> {
    let mut set: JoinSet<Result<PlanDetail, LoadError>> = JoinSet::new();
    let plans = get_all_retailer_plans(client).await?;

    for (plan_id, url) in plans {
        let client = client.clone();
        set.spawn(async move {
            let path = PathBuf::from(CACHE_DIR).join(format!("{plan_id}.json"));

            let plan = if tokio::fs::try_exists(&path).await? {
                let body = tokio::fs::read_to_string(&path).await?;
                let res = serde_json::from_str::<PlanDetailResponse>(&body)?;
                res.data
            } else {
                get_plan_detail(&client, &url, &plan_id).await?
            };
            // let tariffs = plan.electricity_contract.build_tariff().unwrap();
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
    use bill_pricer::{pricer::price, tariff::ConsumptionPeriod};
    use chrono_tz::Tz;
    use domain::meter::{MeterMeasure, PricingInput};
    use gen_meter::generator::{GeneratorConfig, generate_smart_meter};
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use std::str::FromStr;

    use super::*;
    fn priced(meter: &[MeterMeasure], zone: &str) -> PricingInput {
        PricingInput::new(tz(zone), meter)
    }

    fn tz(name: &str) -> Tz {
        Tz::from_str(name).unwrap()
    }

    #[test]
    fn parses_origin_multiflat_vic_offer() {
        let json = include_str!("../data/OR2810912MS@VEC.json");
        let offer = serde_json::from_str::<PlanDetailResponse>(json).unwrap();

        let tariffs = offer.data.electricity_contract.build_tariff().unwrap();
        assert_eq!(tariffs.len(), 1);
        let tariff = tariffs.first().unwrap();

        assert_eq!(tariff.supply_rate(), Decimal::try_from(1.0939).unwrap());
        assert_eq!(tariff.consumption_period(), Some(ConsumptionPeriod::Day));

        let smart_meter = generate_smart_meter(
            &GeneratorConfig {
                start: "2026-01-01 00:00:00".into(),
                end: "2026-01-02 00:00:00".into(),
                frequency: 60,
                daily_kwh: dec!(24),
                ..Default::default()
            },
            "Australia/Sydney",
        )
        .unwrap();
        // 11.78*0.2894 +12.22*0.2894 + 1.0939 = 8.0395 = 8.04 (roun 2)
        let pricing_input = priced(&smart_meter, "Australia/Sydney");

        assert_eq!(
            price(tariff, &pricing_input).unwrap().total_bill,
            dec!(8.04)
        );
    }
}
