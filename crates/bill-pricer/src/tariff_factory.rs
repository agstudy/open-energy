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
    /// Builds a falt tariff with the given flat rate and daily supply rate.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`]: direct use of tariffbuilder
    ///
    ///
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

    /// Builds a falt tariff with export tariff.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`]: direct use of tariffbuilder
    ///
    ///
    pub fn flat_export(
        flat_rate: Decimal,
        supply_rate: Decimal,
        export_rate: Decimal,
    ) -> Result<Tariff, TariffError> {
        TariffBuilder::default()
            .daily_supply(supply_rate)
            .rate_period(|s| {
                s.rates(&[(flat_rate, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })?
            .export_rate_period(|s| {
                s.rates(&[(export_rate, dec!(0))]).time_band(
                    HourMinute::min(),
                    HourMinute::max(),
                    None,
                )
            })?
            .build()
    }

    /// Builds a time-of-use tariff with the given peak and off-peak windows
    /// and daily supply rate.
    ///
    /// # Errors
    ///
    /// Returns [`TariffError`]: direct use of tariffbuilder
    ///
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
  "import": [
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
  "export": null,
  "discount": null,
  "supply_rate": "1.0",
  "cons_period": null
}"#;
        assert_eq!(
            actual.replace("\n", "").replace(" ", "").trim(),
            expected.replace("\n", "").replace(" ", "").trim()
        );
    }

    #[test]
    fn test_deserialize_flat_tariff() {
        use serde_json::json;
        let flat_json = json!(
          {"import": [
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
          "export": null,
          "discount": null,
          "supply_rate": "1.0",
          "cons_period": null,
        });
        let actual: Tariff = serde_json::from_value(flat_json).unwrap();
        assert_eq!(actual, TariffFactory::flat(dec!(0.1), dec!(1.0)).unwrap());
    }
}
