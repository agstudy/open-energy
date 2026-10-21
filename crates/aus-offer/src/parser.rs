use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::cdr_contract::{ParseContractError, PricingModel};
use crate::cdr_response::PlanDetailResponse;
use crate::cdr_tariff::{PeriodTariffError, RateBlockUType};
use rayon::prelude::*;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq, Hash)]
pub enum UnsupportedReason {
    #[error("pricing model")]
    Pricing(PricingModel),
    #[error("Type not implemented")]
    BlockType(RateBlockUType),
    #[error("Solar model not supported")]
    Solar,
}

// --errors
#[derive(Debug, thiserror::Error)]
pub enum FailureReason {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Tariff(#[from] PeriodTariffError),
}

impl FailureReason {
    #[must_use]
    pub fn key(&self) -> String {
        match self {
            Self::Json(_) => "json".into(),
            Self::Io(_) => "io".into(),
            Self::Tariff(t) => format!("tariff/{t}"),
        }
    }
}

pub enum FileOutcome {
    // with how many tariffs it produced
    Converted(PricingModel, usize),
    // the plan is valid, but our model can’t price it (demand, quota, several periods, a gap).
    Unsupported(UnsupportedReason),
    Failed(FailureReason),
}

#[must_use]
pub fn parse_offer_file(path: &Path) -> FileOutcome {
    let body = match std::fs::read_to_string(path) {
        Ok(b) => b,
        Err(e) => return FileOutcome::Failed(FailureReason::Io(e)),
    };
    let res = match serde_json::from_str::<PlanDetailResponse>(&body) {
        Ok(b) => b,
        Err(e) => return FileOutcome::Failed(FailureReason::Json(e)),
    };
    let pricing_model = res.data.electricity_contract.pricing_model.clone();
    match res.data.electricity_contract.build_tariff() {
        Ok(pt) => FileOutcome::Converted(pricing_model, pt.import.len()),
        Err(e) => match e {
            ParseContractError::NotImplemented(p) => {
                FileOutcome::Unsupported(UnsupportedReason::Pricing(p))
            }
            ParseContractError::ParseTariff(PeriodTariffError::NotImplemented(bt)) => {
                FileOutcome::Unsupported(UnsupportedReason::BlockType(bt))
            }
            ParseContractError::ParseTariff(other) => {
                eprintln!("{}: {other}", path.display());
                FileOutcome::Failed(FailureReason::Tariff(other))
            }
            ParseContractError::UnsupportedMultipleOptions => {
                eprintln!("unsuported multiple options {}", path.display());
                FileOutcome::Unsupported(UnsupportedReason::Solar)
            }
        },
    }
}

#[derive(Debug, Default)]
pub struct ParseStats {
    pub converted: HashMap<PricingModel, usize>,
    pub unsupported: HashMap<UnsupportedReason, usize>,
    pub failed: HashMap<String, usize>,
}

impl ParseStats {
    pub fn record(&mut self, result: FileOutcome) {
        match result {
            FileOutcome::Converted(model, _) => *self.converted.entry(model).or_default() += 1,
            FileOutcome::Unsupported(reason) => *self.unsupported.entry(reason).or_default() += 1,
            FileOutcome::Failed(reason) => *self.failed.entry(reason.key()).or_default() += 1,
        }
    }

    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        for (model, count) in other.converted {
            *self.converted.entry(model).or_default() += count;
        }

        for (reason, count) in other.unsupported {
            *self.unsupported.entry(reason).or_default() += count;
        }
        for (reason, count) in other.failed {
            *self.failed.entry(reason).or_default() += count;
        }

        self
    }
}

pub fn parse_offers(files: &[PathBuf]) -> ParseStats {
    println!("nb of files : {}", files.len());
    files
        .par_iter()
        .map(|p| parse_offer_file(p))
        .fold(ParseStats::default, |mut acc, outcome| {
            acc.record(outcome);
            acc
        })
        .reduce(ParseStats::default, ParseStats::merge)
}
