use crate::tariff::ConsumptionPeriod;
use crate::tariff::Instant;
use crate::tariff::PricingError;
use crate::tariff::Tariff;
use crate::tariff::TariffDirection;

use chrono::Datelike;
use chrono::NaiveDate;
use domain::meter::PricingInput;
use domain::meter::Reading;
use rust_decimal::Decimal;
use std::fmt;

struct Acc {
    import_price: Decimal,
    import_kwh: Decimal,
    export_price: Decimal,
    export_kwh: Decimal,
    prev_date: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct PricingResult {
    pub import: Decimal,
    pub import_kwh: Decimal,
    pub export: Decimal,
    pub export_kwh: Decimal,
    pub fixed_bill: Decimal,
    pub variable_bill: Decimal,
    pub total_bill: Decimal,
}

impl std::fmt::Display for PricingResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Pricing Result")?;
        writeln!(
            f,
            "  Import:  {:>12} ({:>10} kWh)",
            self.import, self.import_kwh
        )?;
        writeln!(
            f,
            "  Export:  {:>12} ({:>10} kWh)",
            self.export, self.export_kwh
        )?;
        writeln!(f, "  Fixed:   {:>12}", self.fixed_bill)?;
        writeln!(f, "  Variable:{:>12}", self.variable_bill)?;
        write!(f, "  Total:   {:>12}", self.total_bill)
    }
}

fn period_resets(period: ConsumptionPeriod, prev: Option<NaiveDate>, today: NaiveDate) -> bool {
    let Some(prev) = prev else { return true };
    if prev.year() != today.year() {
        return true;
    }

    match period {
        ConsumptionPeriod::Day => prev != today,
        ConsumptionPeriod::Month => prev.month() != today.month(),
        ConsumptionPeriod::BiMonthly => (prev.month() - 1) >> 1 != (today.month() - 1) >> 1,
        ConsumptionPeriod::Quarterly => (prev.month() - 1) / 3 != (today.month() - 1) / 3,
    }
}

fn reset_acc(r: &Reading, consumption_period: Option<ConsumptionPeriod>, acc: &mut Acc) {
    if let Some(period) = consumption_period {
        let reset = period_resets(period, acc.prev_date, r.local.date);
        if reset {
            acc.import_kwh = Decimal::ZERO;
            acc.export_kwh = Decimal::ZERO;
        }
        acc.prev_date = Some(r.local.date);
    }
}

fn price_variable(tariff: &Tariff, pricing_input: &PricingInput) -> Result<Acc, PricingError> {
    let mut acc = Acc {
        import_price: Decimal::ZERO,
        import_kwh: Decimal::ZERO,
        prev_date: None,
        export_kwh: Decimal::ZERO,
        export_price: Decimal::ZERO,
    };
    for r in pricing_input.readings() {
        reset_acc(r, tariff.consumption_period(), &mut acc);
        // import
        let instant = Instant {
            hm: r.local.hour_minute,
            weekday: r.local.weekday,
            md: r.local.month_day,
        };

        let import_cost = tariff.cost_at(
            TariffDirection::Import,
            &instant,
            acc.import_kwh,
            r.meter.import,
        )?;
        acc.import_price += import_cost;
        acc.import_kwh += r.meter.import;

        // export
        if let Some(export) = r.meter.export {
            let export_cost =
                tariff.cost_at(TariffDirection::Export, &instant, acc.export_kwh, export)?;
            acc.export_price += export_cost;
            acc.export_kwh += export;
        }
    }

    Ok(acc)
}
/// Prices the given input against a tariff.
///
/// # Errors
///
/// Returns [`PricingError`] if the tariff does not fully cover the
/// pricing input's time range, or if any day is not covered by
/// non-overlapping bands.
pub fn price(tariff: &Tariff, pricing_input: &PricingInput) -> Result<PricingResult, PricingError> {
    if pricing_input.is_empty() {
        return Err(PricingError::NoData);
    }

    let supply_charge = tariff.supply_rate() * Decimal::from(pricing_input.days());
    let result = price_variable(tariff, pricing_input)?;
    let fixed = supply_charge.round_dp(2);
    let variable = (result.import_price - result.export_price).round_dp(2);
    let total = fixed + variable;

    Ok(PricingResult {
        fixed_bill: fixed,
        export: result.export_price.round_dp(2),
        import_kwh: result.import_kwh.round_dp(2),
        import: result.import_price.round_dp(2),
        variable_bill: variable,
        export_kwh: result.export_kwh.round_dp(2),
        total_bill: total,
    })
}

#[cfg(test)]
mod tests {

    use crate::tests_utils::str_to_native_date;

    use super::*;

    #[test]
    pub fn test_period_resets() {
        // same day , daily
        assert!(!period_resets(
            ConsumptionPeriod::Day,
            Some(str_to_native_date("2026-04-01")),
            str_to_native_date("2026-04-01")
        ));
        // next day , daily
        assert!(period_resets(
            ConsumptionPeriod::Day,
            Some(str_to_native_date("2026-04-01")),
            str_to_native_date("2026-04-02")
        ));

        // next month monthly
        assert!(period_resets(
            ConsumptionPeriod::Month,
            Some(str_to_native_date("2026-04-01")),
            str_to_native_date("2026-05-01")
        ));
        // split happens in 1 May
        assert!(period_resets(
            ConsumptionPeriod::BiMonthly,
            Some(str_to_native_date("2026-04-01")),
            str_to_native_date("2026-05-01")
        ));
        // no split , quarterly
        assert!(!period_resets(
            ConsumptionPeriod::Quarterly,
            Some(str_to_native_date("2026-04-01")),
            str_to_native_date("2026-06-30")
        ));

        // exaclty split day, quartrely
        for period in [
            ConsumptionPeriod::Day,       // every day
            ConsumptionPeriod::Month,     // every regular month
            ConsumptionPeriod::BiMonthly, //Mar 1 / May 1 / Jul 1 / Sep 1 / Nov 1 / Jan 1
            ConsumptionPeriod::Quarterly, //Apr 1 / Jul 1 / Oct 1 / Jan 1
        ] {
            assert!(period_resets(
                period,
                Some(str_to_native_date("2026-06-30")),
                str_to_native_date("2026-07-01")
            ));
        }
    }
}
