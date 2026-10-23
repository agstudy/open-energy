use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::cdr_contract::{BasePricingModel, ParseContractError, UnsupportedReason};
use crate::cdr_response::PlanDetailResponse;
use crate::cdr_tariff::PeriodTariffError;
use rayon::prelude::*;
use rust_decimal::Decimal;

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OfferCategory {
    pub season: usize,
    pub model: BasePricingModel,
    pub has_controlled_load: bool,
    pub has_solar: bool,
}

pub enum FileOutcome {
    // with how many tariffs it produced
    Converted(OfferCategory),
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
        Ok(pt) => FileOutcome::Converted(OfferCategory {
            season: pt.import_tariff().len(),
            model: pricing_model.base(),
            has_controlled_load: pricing_model.has_controlled_load(),
            has_solar: !pt.export_tariff().is_empty(),
        }),
        Err(e) => match e {
            ParseContractError::NotImplemented(p) => {
                FileOutcome::Unsupported(UnsupportedReason::Pricing(p))
            }
            ParseContractError::ParseTariff(other) => {
                FileOutcome::Failed(FailureReason::Tariff(other))
            }
            ParseContractError::Unsupported(reason) => FileOutcome::Unsupported(reason),
        },
    }
}

#[derive(Debug, Default)]
pub struct ParseStats {
    pub converted: HashMap<OfferCategory, usize>,
    pub unsupported: HashMap<UnsupportedReason, usize>,
    pub failed: HashMap<String, usize>,
}

// Parses every file in `files` in parallel and aggregates the outcomes.
///
/// Each file is parsed independently via [`parse_offer_file`]; results are
/// accumulated per rayon worker and merged into a single [`ParseStats`].
/// Failures are counted, not propagated — the caller inspects `ParseStats`
/// for the classification of every file.
pub fn parse_offers(files: &[PathBuf]) -> ParseStats {
    println!("nb of files : {}", files.len());
    files
        .par_iter()
        .map(|p| parse_offer_file(p))
        .fold(ParseStats::default, |mut acc, outcome| {
            acc.record(outcome);
            acc
        })
        .reduce(ParseStats::default, ParseStats::totals)
}

/// --- Report  
impl ParseStats {
    pub fn record(&mut self, result: FileOutcome) {
        match result {
            FileOutcome::Converted(category) => {
                *self.converted.entry(category).or_default() += 1;
            }
            FileOutcome::Unsupported(reason) => *self.unsupported.entry(reason).or_default() += 1,
            FileOutcome::Failed(reason) => *self.failed.entry(reason.key()).or_default() += 1,
        }
    }

    #[must_use]
    pub fn totals(mut self, other: Self) -> Self {
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

    fn aggregate<K: Ord>(
        &self,
        key: impl Fn(&OfferCategory) -> K,
        filter: impl Fn(&OfferCategory) -> bool,
    ) -> BTreeMap<K, usize> {
        let mut result = BTreeMap::new();
        for (categ, value) in &self.converted {
            if !filter(categ) {
                continue;
            }
            *result.entry(key(categ)).or_default() += value;
        }
        result
    }

    #[must_use]
    pub fn by_season(&self) -> BTreeMap<usize, usize> {
        self.aggregate(|c| c.season, |_| true)
    }

    #[must_use]
    pub fn by_model_season(&self) -> BTreeMap<(BasePricingModel, usize), usize> {
        self.aggregate(|c| (c.model, c.season), |_| true)
    }

    #[must_use]
    pub fn with_solar(&self) -> BTreeMap<(BasePricingModel, usize), usize> {
        self.aggregate(|c| (c.model, c.season), |x| x.has_solar)
    }
    #[must_use]
    pub fn with_controlled_load(&self) -> BTreeMap<(BasePricingModel, usize), usize> {
        self.aggregate(|c| (c.model, c.season), |x| x.has_controlled_load)
    }
}

impl std::fmt::Display for ParseStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let converted: usize = self.converted.values().sum();
        let unsupported: usize = self.unsupported.values().sum();
        let failed: usize = self.failed.values().sum();
        let with_solar: usize = self
            .converted
            .iter()
            .filter(|(c, _)| c.has_solar)
            .map(|(_, n)| n)
            .sum();
        let with_cload: usize = self
            .converted
            .iter()
            .filter(|(c, _)| c.has_controlled_load)
            .map(|(_, n)| n)
            .sum();
        let total = converted + unsupported + failed;

        writeln!(f, "Parse summary ({total} files)")?;

        writeln!(
            f,
            "  converted:   {converted:>6} ({}%)",
            pct(converted, total)
        )?;
        writeln!(
            f,
            "  unsupported: {unsupported:>6} ({}%)",
            pct(unsupported, total)
        )?;
        writeln!(f, "  failed:      {failed:>6} ({}%)", pct(failed, total))?;
        writeln!(
            f,
            "  solar:      {with_solar:>6} ({}%)",
            pct(with_solar, total)
        )?;
        writeln!(
            f,
            "  controlled load:      {with_cload:>6} ({}%)",
            pct(with_cload, total)
        )?;

        if !self.by_season().is_empty() {
            writeln!(f, "\nConverted (seasonal breakdown):")?;
            for (season, count) in self.by_season() {
                writeln!(f, "  {count:>6}  {season}-(s)")?;
            }
        }

        if !self.converted.is_empty() {
            writeln!(f, "\nConverted (model/season breakdown):")?;
            for (season, count) in self.by_model_season() {
                writeln!(f, "  {count:>6}  {season:?}-(s)")?;
            }
        }

        if !self.converted.is_empty() {
            writeln!(f, "\nSolar export tariffs per plan")?;
            writeln!(f, "\nConverted (model/solar-season breakdown):")?;
            for (season, count) in self.with_solar() {
                writeln!(f, "  {count:>6}  {season:?}-(s)")?;
            }
        }

        if !self.converted.is_empty() {
            writeln!(f, "\nControlled load:")?;
            writeln!(f, "\nConverted (model/season breakdown):")?;
            for (season, count) in self.with_controlled_load() {
                writeln!(f, "  {count:>6}  {season:?}-(s)")?;
            }
        }

        if !self.unsupported.is_empty() {
            writeln!(f, "\nUnsupported:")?;
            write_sorted(f, &self.unsupported)?;
        }

        if !self.failed.is_empty() {
            writeln!(f, "\nFailed:")?;
            write_sorted(f, &self.failed)?;
        }

        Ok(())
    }
}

fn pct(n: usize, total: usize) -> Decimal {
    if total == 0 {
        Decimal::ZERO
    } else {
        (Decimal::from(100 * n) / Decimal::from(total)).round_dp(2)
    }
}

fn write_sorted<K: std::fmt::Debug>(
    f: &mut std::fmt::Formatter<'_>,
    map: &std::collections::HashMap<K, usize>,
) -> std::fmt::Result {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|a, b| {
        b.1.cmp(a.1)
            .then_with(|| format!("{:?}", a.0).cmp(&format!("{:?}", b.0)))
    });
    for (key, count) in entries {
        writeln!(f, "  {count:>6}  {key:?}")?;
    }
    Ok(())
}
