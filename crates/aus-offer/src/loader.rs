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
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    ParseError(#[from] ParseContractError),
}
/// save plan by id
async fn save_plan(body: &str, plan_id: &str) -> Result<(), LoadError> {
    tokio::fs::create_dir_all(CACHE_DIR).await?;
    let path = PathBuf::from(CACHE_DIR).join(format!("{plan_id}.json"));
    fs::write(&path, body).await?;
    Ok(())
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
/// Fetches a page of plans for a retailer from the given base URL.
///
/// # Errors
///
/// Returns [`LoadError`] if:
/// - an HTTP request fails or the response status is not success
///   ([`LoadError::Reqwest`]),
/// - a response body cannot be deserialised into [`PlanResponseDto`]
///   ([`LoadError::JsonInUrl`]).
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
/// Fetches the plan catalogue for all retailers.
///
/// # Errors
///
/// Returns [`LoadError`] if:
/// - any underlying HTTP request fails ([`LoadError::Reqwest`]),
/// - a response body cannot be deserialised ([`LoadError::JsonInUrl`]),
/// - a spawned fetch task panics or is cancelled ([`LoadError::Join`]).
pub async fn get_all_retailer_plans(client: &Client) -> Result<HashMap<String, String>, LoadError> {
    let mut set: JoinSet<Result<HashMap<String, String>, LoadError>> = JoinSet::new();

    for &url in ENERGY_BASE_URIS.values() {
        // Arc handle for the task (refcount bump, not a Client clone)
        let client = client.clone();
        set.spawn(async move {
            let plans = get_retailer_plans(&client, url, 1000).await?;

            Ok(plans
                .into_iter()
                .map(|p| (p.plan_id, url.to_string()))
                .collect::<HashMap<_, _>>())
        });
    }

    let mut result: HashMap<String, String> = HashMap::new();
    while let Some(res) = set.join_next().await {
        let partial = res??;
        result.extend(partial);
    }

    Ok(result)
}

/// Fetches the detailed record for a single plan.
///
/// # Errors
///
/// Returns [`LoadError`] if:
/// - the HTTP request fails or the response status is not success  ([`LoadError::Reqwest`]),
/// - the response body cannot be deserialised into [`PlanDetailResponse`]  ([`LoadError::JsonInUrl`]).
/// - the response cannot be saved to the on-disk cache   ([`LoadError::Io`]).
pub async fn get_plan_detail(
    client: &Client,
    retailer_base_url: &str,
    plan_id: &str,
) -> Result<PlanDetail, LoadError> {
    let url = format!("{retailer_base_url}cds-au/v1/energy/plans/{plan_id}");
    let body = fetch_url(client, &url, 3).await?;

    save_plan(&body, plan_id).await?;

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

/// Parses an offer file on disk into a list of tariffs.
/// # Errors
///
/// Returns [`LoadError`] if:
/// - the file cannot be read ([`LoadError::Io`]),
/// - the file's contents are not valid JSON for a [`PlanDetailResponse`]
///   ([`LoadError::Json`]),
/// - the embedded electricity contract is invalid
///   ([`LoadError::ParseError`] wrapping [`ParseContractError`]).
pub fn parse_offer_file(path: &Path) -> Result<Vec<Tariff>, LoadError> {
    let body = std::fs::read_to_string(path)?;
    let res = serde_json::from_str::<PlanDetailResponse>(&body)?;
    let tariff = res.data.electricity_contract.build_tariff()?;
    Ok(tariff)
}

/// Fetches the details for every plan across all retailers.
///
/// Uses a per-plan on-disk cache under [`CACHE_DIR`]; cached plans are read
/// from disk instead of being re-fetched.
///
/// # Errors
///
/// Returns [`LoadError`] if:
/// - the initial catalogue fetch fails ([`LoadError::Reqwest`],
///   [`LoadError::JsonInUrl`], [`LoadError::Join`]),
/// - a cached plan file cannot be read ([`LoadError::Io`]) or its contents
///   are not valid JSON ([`LoadError::Json`]),
/// - fetching or decoding an uncached plan fails ([`LoadError::Reqwest`],
///   [`LoadError::JsonInUrl`]),
/// - a spawned task panics or is cancelled ([`LoadError::Join`]).
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
    use domain::{
        meter::{MeterMeasure, PricingInput},
    };
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
        let json = include_str!("../data/flat/OR2810912MS@VEC.json");
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

    #[test]
    fn parses_tou_overlapp_tariff() {
        let json = include_str!("../data/tou/1ST764143MS@VEC.json");
        let offer = serde_json::from_str::<PlanDetailResponse>(json).unwrap();

        let tariffs = offer.data.electricity_contract.build_tariff().unwrap();
        assert_eq!(tariffs.len(), 1);
        let tariff = tariffs.first().unwrap();

        assert_eq!(tariff.supply_rate(), Decimal::try_from(1.498).unwrap());
        assert_eq!(tariff.consumption_period(), None);

        //This plan: the converted weekday windows are [00:00–08:59, 09:00–20:59, 21:00–23:59].
        let windows = tariff
            .import_tariff()
            .iter()
            .map(|r| format!("{}-{}", r.time_band.start(), r.time_band.end()))
            .collect::<Vec<String>>();

        assert_eq!(windows,["09:00-20:59", "21:00-23:59", "00:00-08:59", "00:00-23:59"]);
    }

    #[test]
    fn parses_tou_start_bigger_end_tariff() {
        let json = include_str!("../data/tou/RED552217MBE13@EME.json");
        let offer = serde_json::from_str::<PlanDetailResponse>(json).unwrap();

        let tariffs = offer.data.electricity_contract.build_tariff().unwrap();
        assert_eq!(tariffs.len(), 1);
        let tariff = tariffs.first().unwrap();

        assert_eq!(tariff.supply_rate(), Decimal::try_from(1.30).unwrap());
        assert_eq!(tariff.consumption_period(), None);
    }

    #[test] 
    fn parses_tou_missing_tou() {
        let json = include_str!("../data/tou/FLO806120MR@VEC.json");
        let offer = serde_json::from_str::<PlanDetailResponse>(json).unwrap();

        let tariffs = offer.data.electricity_contract.build_tariff().unwrap();
        assert_eq!(tariffs.len(), 1);
        let tariff = tariffs.first().unwrap();

        assert_eq!(tariff.supply_rate(), Decimal::try_from(1.7035).unwrap());
        assert_eq!(tariff.consumption_period(), None);
    }

}
