use serde::Deserialize;

use crate::{
    cdr_domain::{SingleRates, TariffUType, TimeOfUseRates},
    cdr_tariff::PeriodTariffError,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolarTariff {
    pub scheme: SolarScheme,
    pub payer_type: PayerType,
    pub tariff_u_type: TariffUType,
    #[serde(rename = "singleTariff")]
    pub single_rate: Option<SingleRates>,
    #[serde(rename = "timeVaryingTariffs")]
    pub time_of_use_rates: Option<Vec<TimeOfUseRates>>,
}

/// The applicable scheme.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolarScheme {
    #[serde(rename = "PREMIUM")]
    Premium,
    #[serde(rename = "CURRENT")]
    Current,
    #[serde(rename = "VARIABLE")]
    Variable,
    #[serde(rename = "OTHER")]
    Other,
}

/// The type of the payer.
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PayerType {
    #[serde(rename = "GOVERNMENT")]
    Government,
    #[serde(rename = "RETAILER")]
    Retailer,
}

impl SolarTariff {
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.scheme == SolarScheme::Premium || self.payer_type == PayerType::Government
    }

    /// Returns the single-rate payload for this feed-in entry.
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError::MissingField`] if `singleTariff` is absent.
    pub fn get_single(&self) -> Result<&SingleRates, PeriodTariffError> {
        self.single_rate
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("singleTariff"))
    }

    /// Returns the time-of-use payload for this feed-in entry.
    ///
    /// # Errors
    ///
    /// Returns [`PeriodTariffError::MissingField`] if `timeVaryingTariffs` is absent.
    pub fn get_tou(&self) -> Result<&Vec<TimeOfUseRates>, PeriodTariffError> {
        self.time_of_use_rates
            .as_ref()
            .ok_or(PeriodTariffError::MissingField("timeVaryingTariffs"))
    }
}
