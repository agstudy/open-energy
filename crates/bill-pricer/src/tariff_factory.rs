use domain::HourMinute;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

use crate::tariff::TariffBuilder;
use crate::tariff::{Tariff, TariffError};

pub struct TariffFactory;

#[derive(Debug, Clone)]
pub struct Window {
    pub rate: Decimal,
    pub start: HourMinute,
}

impl TariffFactory {
    pub fn flat(flat_rate: Decimal, supply_rate: Decimal) -> Result<Tariff, TariffError> {
        TariffBuilder::default()
            .daily_supply(supply_rate)
            .rate_period(|s| {
                s.rates(&[(flat_rate, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })?
            .build()
    }
    pub fn time_of_use(
        peak: &Window,
        off_peak: &Window,
        supply_rate: Decimal,
    ) -> Result<Tariff, TariffError> {
        let mut builder = TariffBuilder::default().daily_supply(supply_rate);

        // Off-peak runs from off_peak.start until peak begins (may cross midnight).
        for (start, end) in off_peak.start.split_midnight(peak.start.prev()) {
            builder = builder.rate_period(|s| {
                s.rates(&[(off_peak.rate, dec!(0))])
                    .time_band(start, end, None)
            })?;
        }
        // Peak runs from peak.start until off-peak resumes (may cross midnight).
        for (start, end) in peak.start.split_midnight(off_peak.start.prev()) {
            builder = builder
                .rate_period(|s| s.rates(&[(peak.rate, dec!(0))]).time_band(start, end, None))?;
        }

        builder.build()
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn test_serialize_flat_tariff() {
        let flat_tariff = TariffFactory::flat(dec!(0.1), dec!(1.0)).unwrap();
        let actual = serde_json::to_string_pretty(&flat_tariff).unwrap();

        let expected = r#"{
  "import_tariff": [
    {
      "rates": [
        {
          "rate": "0.1",
          "lower_band": "0"
        }
      ],
      "time_band": {
        "start": {
          "hour": 0,
          "minute": 0
        },
        "end": {
          "hour": 23,
          "minute": 59
        },
        "days_of_week": null
      }
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
        let flat_json = json!(
          {"import_tariff": [
            {
              "rates": [
                {
                  "rate": "0.1",
                  "lower_band": "0.0"
                }
              ],
              "time_band": {
                "start": {
                  "hour": 0,
                  "minute": 0
                },
                "end": {
                  "hour": 23,
                  "minute": 59
                },
                "days_of_week": null
              }
            }
          ],
          "export_tariff": null,
          "discount": null,
          "supply_rate": "1.0"
        });
        let actual: Tariff = serde_json::from_value(flat_json).unwrap();
        assert_eq!(actual, TariffFactory::flat(dec!(0.1), dec!(1.0)).unwrap());
    }
}
