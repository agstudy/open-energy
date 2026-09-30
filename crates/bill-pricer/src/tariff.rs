use crate::models::{END_OF_DAY, HourMinute, InvalidHourMinute, START_OF_DAY};
use crate::models::{InvalidTimeBand, TimeBand};
use chrono::{DateTime, Timelike, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct RateBlock {
    pub rate: Decimal,
    pub lower_band: Decimal,
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
    pub time_band: TimeBand,
}

impl RatePeriod {
    pub fn applies_at(&self, at: &HourMinute) -> bool {
        self.time_band.start() <= *at && *at <= self.time_band.end()
    }

    pub fn new(rate: Decimal, time_band: TimeBand) -> Self {
        RatePeriod {
            rates: vec![RateBlock {
                rate,
                lower_band: dec!(0.0),
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

#[derive(Debug, thiserror::Error)]
pub enum TariffError {
    #[error("bands leave a gap starting at {0:?}")]
    Gap(HourMinute),
    #[error("bands overlap at {0:?}")]
    Overlap(HourMinute),
    #[error("tier thresholds out of order")]
    TierOrder,
    #[error("Missing rates")]
    EmptyRates,
    #[error(transparent)]
    InvalidTimeBand(#[from] InvalidTimeBand),
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

    pub fn validate(&mut self) -> Result<(), TariffError> {
        validate_tariff(&mut self.import_tariff)?;
        if let Some(export) = &mut self.export_tariff {
            validate_tariff(export)?;
        }
        Ok(())
    }
}

/// Validate that a set of rate periods fully covers a day with no overlaps
/// and monotonic tier thresholds.
///
/// ```text
/// 1. sort tp by s_i                      -- O(n log n)
/// 2. cursor ← 0
/// 3. for each (s_i, e_i) in sorted order:
///      check_tier_order(rates_i)?
///      if s_i > cursor:  return Gap(cursor)
///      if s_i < cursor:  return Overlap(s_i)
///      cursor ← e_i + 1
/// 4. if cursor ≠ 1440: return Gap(cursor)
/// 5. return Ok
/// ```
fn validate_tariff(rps: &mut [RatePeriod]) -> Result<(), TariffError> {
    rps.sort_by_key(|rp| rp.time_band.start());

    let mut cursor: u32 = 0;
    for rp in rps.iter() {
        check_tier_order(&rp.rates)?;
        let tb = &rp.time_band;
        if tb.start().minute_of_day() > cursor {
            return Err(TariffError::Gap(HourMinute::from_minute_of_day(cursor)));
        } else if tb.start().minute_of_day() < cursor {
            return Err(TariffError::Overlap(HourMinute::from_minute_of_day(cursor)));
        }
        // `end()` is inclusive, so the next band must start at end+1 to be gap-free.
        cursor = tb.end().minute_of_day() + 1;
    }
    if cursor != 1440 {
        return Err(TariffError::Gap(HourMinute::from_minute_of_day(cursor)));
    }

    Ok(())
}

fn check_tier_order(rates: &[RateBlock]) -> Result<(), TariffError> {
    let mut it = rates.iter().map(|v| v.lower_band);

    let Some(first) = it.next() else {
        return Err(TariffError::EmptyRates);
    };

    if first != Decimal::ZERO {
        return Err(TariffError::TierOrder);
    }

    let mut prev = first;
    for cur in it {
        if prev >= cur {
            return Err(TariffError::TierOrder);
        }
        prev = cur;
    }
    Ok(())
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
                Ok(RatePeriod::new(rate, tb))
            })
            .collect()
    }

    pub fn flat(flat_rate: Decimal, supply_rate: Decimal) -> Tariff {
        Tariff {
            import_tariff: vec![RatePeriod {
                rates: vec![RateBlock {
                    rate: flat_rate,
                    lower_band: dec!(0),
                }],
                time_band: TimeBand::new(START_OF_DAY, END_OF_DAY, None).unwrap(),
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
        assert_eq!(actual, TariffFactory::flat(dec!(0.1), dec!(1.0)));
    }
}
