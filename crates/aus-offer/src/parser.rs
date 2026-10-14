// crates/offers/src/parser.rs
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use thiserror::Error;

// ---------- Domain ----------

#[derive(Debug, Clone)]
pub struct Offer {
    pub offer_id: String,
    pub offer_name: String,
    pub retailer_name: String,
    pub distributor: String,
    pub release_date: NaiveDate,
    pub eligibility: Option<String>,
    pub contract_length: Option<String>,
    pub exit_fee: Decimal,
    pub gst_included: bool,
    pub gst_percent: Decimal,
    pub has_solar: bool,
    pub has_controlled_load: bool,
    pub discounts: Vec<Discount>,
    pub fees: Vec<Fee>,
    pub solar: Option<SolarCredit>,
    pub tariff: Tariff,
}

#[derive(Debug, Clone)]
pub struct Discount {
    pub percentage: Decimal,
    pub application_type: String,
    pub categories: Vec<String>,
    pub conditional: bool,
    pub conditions: String,
}

#[derive(Debug, Clone)]
pub struct Fee {
    pub fee_type: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone)]
pub struct SolarCredit {
    pub rate: Decimal,
    pub description: String,
}

// Placeholder — real Tariff lives in bill-pricer.
#[derive(Debug, Clone)]
pub struct Tariff {
    pub daily_charge: Decimal,
    pub rates: Vec<Rate>,
}

#[derive(Debug, Clone)]
pub struct Rate {
    pub rate: Decimal,
    pub threshold: Decimal,
}

// ---------- Errors ----------

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("no offers in file")]
    NoOffers,
    #[error("date: {0}")]
    Date(#[from] chrono::ParseError),
    #[error("unsupported tariff shape: {0}")]
    UnsupportedShape(String),
}

// ---------- DTOs ----------

#[derive(Debug, Deserialize)]
pub struct Root {
    #[serde(rename = "offerDetails")]
    pub offer_details: Vec<OfferDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferDto {
    #[serde(rename = "offerId")]
    pub offer_id: String,
    #[serde(rename = "offerName")]
    pub offer_name: String,
    #[serde(rename = "retailerName")]
    pub retailer_name: String,
    pub distributor: String,

    #[serde(rename = "releaseDate")]
    pub release_date: String,

    #[serde(rename = "eligibilityCriteria", default)]
    pub eligibility: Option<String>,

    #[serde(rename = "contractLength", default)]
    pub contract_length: Option<String>,

    #[serde(rename = "exitFee", default)]
    pub exit_fee: Decimal,

    #[serde(rename = "includeGSTRates", default)]
    pub include_gst_rates: bool,

    #[serde(rename = "GSTPercent", default)]
    pub gst_percent: Decimal,

    #[serde(rename = "isAvailableForSolar", default)]
    pub is_available_for_solar: bool,

    #[serde(rename = "isControlLoad", default)]
    pub is_control_load: bool,

    #[serde(rename = "tariffType")]
    pub tariff_type: String,

    #[serde(rename = "underlyingNetworkTariffType")]
    pub underlying_network_tariff_type: String,

    #[serde(default)]
    pub discounts: Vec<DiscountDto>,
    #[serde(default)]
    pub fees: Vec<FeeDto>,
    #[serde(default)]
    pub solar: Vec<SolarDto>,

    #[serde(rename = "tariffDetails")]
    pub tariff_details: Vec<TariffDetailDto>,
}

#[derive(Debug, Deserialize)]
pub struct DiscountDto {
    #[serde(rename = "discount_percentage", default)]
    pub percentage: Decimal,
    #[serde(rename = "discount_application_type", default)]
    pub application_type: String,
    #[serde(rename = "discount_categories", default)]
    pub categories: Vec<String>,
    #[serde(rename = "discount_type", default)]
    pub discount_type: String,
    #[serde(rename = "discount_conditions", default)]
    pub conditions: String,
}

#[derive(Debug, Deserialize)]
pub struct FeeDto {
    #[serde(rename = "fee_type")]
    pub fee_type: String,
    #[serde(rename = "fee_amount", default)]
    pub amount: Decimal,
}

#[derive(Debug, Deserialize)]
pub struct SolarDto {
    #[serde(default)]
    pub rate: Decimal,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct TariffDetailDto {
    #[serde(rename = "tariff_daily_charge", default)]
    pub daily_charge: Decimal,
    #[serde(rename = "TARIFFS", default)]
    pub tariffs: Vec<TariffBlockDto>,
}

#[derive(Debug, Deserialize)]
pub struct TariffBlockDto {
    #[serde(rename = "RATES", default)]
    pub rates: Vec<RateDto>,
}

#[derive(Debug, Deserialize)]
pub struct RateDto {
    #[serde(rename = "rate_block", default)]
    pub rate: Decimal,
    #[serde(rename = "rate_block_consumption", default)]
    pub threshold: Decimal,
}

// ---------- Mapping ----------

impl TryFrom<OfferDto> for Offer {
    type Error = ParseError;

    fn try_from(dto: OfferDto) -> Result<Self, Self::Error> {
        let release_date = NaiveDate::parse_from_str(&dto.release_date, "%d/%m/%Y")?;

        let tariff = build_tariff(&dto)?;

        Ok(Offer {
            offer_id: dto.offer_id,
            offer_name: dto.offer_name,
            retailer_name: dto.retailer_name,
            distributor: dto.distributor,
            release_date,
            eligibility: dto.eligibility.filter(|s| !s.is_empty()),
            contract_length: dto.contract_length.filter(|s| !s.is_empty()),
            exit_fee: dto.exit_fee,
            gst_included: dto.include_gst_rates,
            gst_percent: dto.gst_percent,
            has_solar: dto.is_available_for_solar,
            has_controlled_load: dto.is_control_load,
            discounts: dto.discounts.into_iter().map(Into::into).collect(),
            fees: dto.fees.into_iter().map(Into::into).collect(),
            solar: dto.solar.into_iter().next().map(Into::into),
            tariff,
        })
    }
}

impl From<DiscountDto> for Discount {
    fn from(d: DiscountDto) -> Self {
        let conditional = d.discount_type.eq_ignore_ascii_case("Conditional");
        Discount {
            percentage: d.percentage,
            application_type: d.application_type,
            categories: d.categories,
            conditional,
            conditions: d.conditions,
        }
    }
}

impl From<FeeDto> for Fee {
    fn from(f: FeeDto) -> Self {
        Fee {
            fee_type: f.fee_type,
            amount: f.amount,
        }
    }
}

impl From<SolarDto> for SolarCredit {
    fn from(s: SolarDto) -> Self {
        SolarCredit {
            rate: s.rate,
            description: s.description,
        }
    }
}

fn build_tariff(dto: &OfferDto) -> Result<Tariff, ParseError> {
    // Only flat shapes for now. Dispatch on underlyingNetworkTariffType later.
    match dto.underlying_network_tariff_type.as_str() {
        "flat" => {
            let detail = dto
                .tariff_details
                .first()
                .ok_or_else(|| ParseError::UnsupportedShape("no tariffDetails".into()))?;
            let rates = detail
                .tariffs
                .iter()
                .flat_map(|t| t.rates.iter())
                .map(|r| Rate {
                    rate: r.rate,
                    threshold: r.threshold,
                })
                .collect();
            Ok(Tariff {
                daily_charge: detail.daily_charge,
                rates,
            })
        }
        other => Err(ParseError::UnsupportedShape(other.into())),
    }
}

// ---------- Entry point ----------

pub fn parse_offer(json: &str) -> Result<Offer, ParseError> {
    let root: Root = serde_json::from_str(json)?;
    let dto = root
        .offer_details
        .into_iter()
        .next()
        .ok_or(ParseError::NoOffers)?;
    Offer::try_from(dto)
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vic_offer() {
        let json = include_str!("../tests/fixtures/vic-2026-07-01.json");
        let offer = parse_offer(json).unwrap();

        assert_eq!(offer.offer_name, "1st Opal, Residential - Anytime");
        assert_eq!(offer.retailer_name, "1st Energy");
        assert_eq!(offer.distributor, "Citipower");
        assert_eq!(
            offer.release_date,
            NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()
        );
        assert!(offer.gst_included);
        assert_eq!(offer.gst_percent, Decimal::from(10));
        assert_eq!(offer.discounts.len(), 1);
        assert_eq!(offer.discounts[0].percentage, Decimal::from(5));
        assert!(offer.discounts[0].conditional);
        assert_eq!(offer.fees.len(), 4);
        assert_eq!(offer.solar.as_ref().unwrap().rate, Decimal::new(5, 1));
        assert_eq!(offer.tariff.daily_charge, Decimal::new(906, 1));
        assert_eq!(offer.tariff.rates[0].rate, Decimal::new(194, 1));
    }
}
