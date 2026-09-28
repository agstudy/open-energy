use super::hour_minute::{HourMinute, InvalidHourMinute};
use super::time_band::{InvalidTimeBand, TimeBand};
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Option<Decimal>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountValueType {
    Percent,
    Absolute,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum DiscountApplicationType {
    Total,
    Energy,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Discount {
    pub value_type: DiscountValueType,
    pub application_type: DiscountApplicationType,
    pub amount: Decimal,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RatePeriod {
    pub rates: Vec<RateBlock>,
    pub time_band: Option<TimeBand>,
}

impl RatePeriod {
    /// Returns true if this period covers `at`. `time_band: None` means
    /// "always applies" (used for flat/non TOU).
    /// Day-of-week matching not yet implemented.
    /// Invariant: `start <= end`. Overnight ranges must be pre-split into two
    /// bands (see `HourMinute::split_midnight`) before constructing a `TimeBand`.
    pub fn applies_at(&self, at: &HourMinute) -> bool {
        match &self.time_band {
            None => true,
            Some(tb) => tb.start() <= *at && *at <= tb.end(),
        }
    }

    pub fn new(rate: Decimal, time_band: Option<TimeBand>) -> Self {
        RatePeriod {
            rates: vec![RateBlock {
                rate,
                lower_band: None,
            }],
            time_band,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Tariff {
    pub import_tariff: Vec<RatePeriod>,
    pub export_tariff: Option<Vec<RatePeriod>>,
    pub discount: Option<Vec<Discount>>,
    pub supply_rate: Decimal,
}

#[derive(Debug, thiserror::Error)]
pub enum PricingError {
    TariffOverlapp,
    NoData,
    NoRates,
    NoTariff,
    InvalidHourMinute(InvalidHourMinute),
    CorruptedData,
}

impl std::fmt::Display for PricingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pricing error")
    }
}

#[derive(Debug, Clone)]
pub struct Window {
    pub rate: Decimal,
    pub start: HourMinute,
}

impl Tariff {
    pub fn rate_at(&self, at: DateTime<Utc>) -> Result<Decimal, PricingError> {
        let at_hm =
            HourMinute::new(at.hour(), at.minute()).map_err(PricingError::InvalidHourMinute)?;

        let matches: Vec<&RatePeriod> = self
            .import_tariff
            .iter()
            .filter(|p| p.applies_at(&at_hm))
            .collect();

        match matches.as_slice() {
            [] => Err(PricingError::NoTariff),
            [period] => period
                .rates
                .first()
                .ok_or(PricingError::NoRates)
                .map(|b| b.rate),
            _ => Err(PricingError::TariffOverlapp),
        }
    }
}

pub struct TariffFactory;

impl TariffFactory {
    fn periods_for_window(
        rate: Decimal,
        start: HourMinute,
        end: HourMinute,
    ) -> Result<Vec<RatePeriod>, InvalidTimeBand> {
        start
            .split_midnight(end)
            .into_iter()
            .map(|(s, e)| {
                let tb = TimeBand::new(s, e, None)?;
                Ok(RatePeriod::new(rate, Some(tb)))
            })
            .collect()
    }

    pub fn flat(flat_rate: Decimal, supply_rate: Decimal) -> Tariff {
        Tariff {
            import_tariff: vec![RatePeriod {
                rates: vec![RateBlock {
                    rate: flat_rate,
                    lower_band: None,
                }],
                time_band: None,
            }],
            export_tariff: None,
            discount: None,
            supply_rate,
        }
    }
    pub fn time_of_use(
        peak: Window,
        off_peak: Window,
        supply_rate: Decimal,
    ) -> Result<Tariff, InvalidTimeBand> {
        let mut import_tariff =
            TariffFactory::periods_for_window(off_peak.rate, off_peak.start, peak.start.prev())?;

        import_tariff.extend(TariffFactory::periods_for_window(
            peak.rate,
            peak.start,
            off_peak.start.prev(),
        )?);

        Ok(Tariff {
            import_tariff,
            export_tariff: None,
            discount: None,
            supply_rate,
        })
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_serialize_flat_tariff() {
        let flat_tariff = TariffFactory::flat(dec!(0.1), dec!(1.0));
        let actual = serde_json::to_string_pretty(&flat_tariff).unwrap();

        let expected = r#"{
  "import_tariff": [
    {
      "rates": [
        {
          "rate": "0.1",
          "lower_band": null
        }
      ],
      "time_band": null
    }
  ],
  "export_tariff": null,
  "discount": null,
  "supply_rate": "1.0"
}"#;
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_deserialize_flat_tariff() {
        use serde_json::json;
        let flat_json = json!({
          "import_tariff": [
            {
              "rates": [
                {
                  "rate": "0.1",
                  "lower_band": null
                }
              ],
              "time_band": null
            }
          ],
          "export_tariff": null,
          "discount": null,
          "supply_rate": "1.0"
        });

        let actual: Tariff = serde_json::from_value(flat_json).unwrap();
        assert_eq!(actual, TariffFactory::flat(dec!(0.1), dec!(1.0)));
    }
}
