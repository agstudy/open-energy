# open-energy — Design Document

This is a living document. Each time a design decision is made for a crate — before or during implementation — it gets folded into that crate's section here. The goal is that a new contributor (especially one adding support for a new country's tariffs) can read this and understand the *why* behind the types, without having to reverse-engineer it from source or ask the maintainer directly.

Keep entries as decisions and rationale, not full transcripts of how the decision was reached.

---

## Vision

open-energy audits a user's full energy profile (smart meter data plus current gas/diesel/fossil usage) and models electrification as a carbon-free solution — either by switching fossil energy sources to electric (solar panels, home battery, EV replacing a diesel/petrol car) or by upgrading to a more efficient electric solution (mainly heat pumps) — for both residential and business users. Electricity offer switching (bill comparison across retailer offers) is a separate, usable-standalone layer, before or after electrification.

The project is meant to be genuinely international from the start — not Australia-only — specifically to invite contribution from developers in other countries (France, Germany, UK, China, US, and beyond) who can add their own local tariff structures without needing to touch the core model or understand another country's conventions.

---

## Crate map (current + planned)

- `meter-parser` — parses smart meter data (NEM12 format, Australia-specific) into a usage profile. **Done.**
- `energy-cli` — CLI entry point, wires crates together for end-to-end usage. **In progress.**
- `battery-simulation` — models battery charge/discharge behavior. Kept out of the public repo for now. Depends on the future `Tariff` type. See its own section below.
- `bill-calculator` — computes the cost of a usage profile against a `Tariff`. **Design in progress — this document's first real section.**
- `offer-scraper` — fetches and normalizes real electricity offers from retailer/regulator APIs (Victoria, NSW/CDR, and eventually others) into a shared `Offer` model. **Not started.**

Deliberately deferred until the core parser → bill-calculator → comparator pipeline is validated by real users on a CLI: database storage, cloud deployment, community contribution infrastructure, any paid-hosting model.

---

## bill-calculator / Tariff model

### What a Tariff is

A tariff is fundamentally **one or more time bands covering the full day**. A flat, single-rate tariff is not a special case — it's simply the degenerate case of one band spanning the whole day. This unification was confirmed by real-world data: Victoria's "Single rate", "Time of use", and "Flexible pricing" tariff types are all structurally the same shape once modeled this way.

Each time band's rate can itself be **flat or tiered** — i.e. one or more consumption tiers within that band, where a tier can have its own rate (including zero). This is not a hypothetical case: a real Victorian offer ("4Free window") gives the first 32 kWh free within an 11am–3pm band, then charges for consumption above that within the same band. Tiering-within-a-band is a first-class requirement, not an edge case.

On top of the time-banded usage charges sits a **daily supply/standing charge**.

**Discounts** can apply to:
- the energy (usage) charge alone, or
- the total bill, computed after energy charge + daily supply charge are combined.

A discount can be a percentage or a flat amount off (needs an enum with both variants). Multiple discounts can stack; each declares which of the two targets (usage-only vs. total) it applies to.

### Import and export symmetry

Solar export pricing uses the *same* time-banded, tiered rate shape as import — confirmed from real data where export pays more at evening peak than at midday. There is no need for a separate "export tariff" shape; `Offer` (see below) simply holds an optional second `Tariff` for export.

### Point-query as the core primitive

`Tariff` should expose a point-in-time query — conceptually `rate_at(timestamp) -> Money` — as its primary primitive, with whole-bill calculation implemented as a sum of point-queries over a billing period. This is what lets:
- `battery-simulation` ask "what's the marginal rate right now" while making dispatch decisions,
- `bill-calculator` sum an entire billing period from the same source of truth,
- the eventual offer-comparison step parallelize cleanly (each `(scenario, offer)` pair is independent — see below), since the calculation is a pure function with no shared mutable state.

### International considerations

The core shape (bands × tiers + supply charge + discounts) holds up across countries already surveyed (Australia, and the general pattern matches France's HP/HC, Germany's day/night, UK Economy 7, China's TOU, and US utility TOU plans).

Known gaps, intentionally deferred rather than solved upfront:
- **Currency** — `Tariff` needs a currency (not assumed AUD). Use a `Money`-style type (`Decimal` + currency) rather than a bare `Decimal`.
- **Tax handling** — GST/VAT/utility-tax conventions vary by country and are sometimes inclusive, sometimes not. Decision: keep `Tariff`'s stored rate as the actual chargeable rate (tax-inclusive by convention), and treat "is this tax-inclusive" as metadata for display/reporting — not something the core calculation engine needs to reason about.
- **Demand charges** (kW peak-based billing, common in US/China commercial tariffs) — not modeled yet. Flagged as a known gap and open contribution area rather than solved now, since it's mostly a commercial/industrial concern rather than residential. Also noted as potentially relevant to battery-simulation later, since a battery can reduce demand-charge exposure by peak-shaving, independent of the energy-arbitrage value it provides against time-of-use rates.
- **Billing cycle length** (monthly vs. quarterly) — doesn't change `Tariff` itself, only how `bill-calculator` aggregates the daily supply charge over a period.

### Explicit non-goals for `Tariff` (kept separate on purpose)

- `Tariff` knows nothing about retailers, plan names, eligibility, or offer expiry — that's the job of `Offer` (see `offer-scraper` section), which wraps an import `Tariff` and an optional export `Tariff`.
- `Tariff` knows nothing about solar, batteries, or heat pumps — those are upstream profile transformers (see `battery-simulation` section) that produce a modified `UsageProfile`, which is then evaluated against a `Tariff` — same as an unmodified profile would be.
- A dedicated `tariff` crate (split out from `bill-calculator`) was considered and explicitly deferred as premature at this stage.

---

## offer-scraper (not started)

Will fetch and normalize offers from per-country/per-region sources into a shared `Offer` model:

```
Offer {
    retailer, plan_id, plan_name, effective_from, eligibility, ...
    import_tariff: Tariff,
    export_tariff: Option<Tariff>,
}
```

Each data source (Victoria, NSW/CDR, future countries) gets its own adapter that normalizes that source's raw schema into `Offer` — the internal model must never mirror any one source's raw JSON shape, since sources genuinely differ (confirmed: Victoria's nested TARIFFS/RATES/TIME_BANDS vs. NSW's older touBlock/blockPeriod shape vs. NSW's newer CDR v3 schema are all structurally different from each other).

Planned scope: retailer base-URI registry, paginated plan listing, per-plan detail fetch, API version negotiation, rate-limit/backoff handling, per-retailer error isolation. Deliberately kept minimal (single binary, simple concurrency) — this crate is also a learning vehicle for async I/O, real-world flaky-network error handling, and messy third-party JSON deserialization, but should not become a distraction from the core product.

---

## battery-simulation

Depends on the shared `Tariff` type to make charge/discharge decisions against time-varying rates (including free/zero-cost windows).

Key modeling decisions:
- **Battery is stateful/cumulative** — the charge/discharge decision at time T depends on state of charge carried over from prior timestamps. This is fundamentally different from solar panels or heat pumps, whose effect on usage at a given instant depends only on that instant (stateless, order-independent, can be computed per-point).
- **Scenario pipeline ordering**: apply solar panel and heat pump adjustments to the raw usage profile first (order-independent, cheap), then run the battery simulation last as a separate sequential pass over the combined adjusted profile. This avoids needing to model complex interactions between technologies directly inside the battery simulator.
- **Known hard case, unresolved**: some users' smart meter data already reflects an existing battery (or solar, or heat pump) installed. NEM12 (and smart meter data generally) only records net import/export at the meter boundary — it has no visibility into behind-the-meter equipment. Reconstructing "true" underlying consumption/generation from a net signal that already includes an existing battery's effect is a hard, only partially solved research problem (related field: non-intrusive load monitoring / behind-the-meter disaggregation). Planned mitigation: ask users directly what equipment they already have, while acknowledging this alone doesn't fully solve reconstructing historical dispatch behavior. Not something to solve before the CLI validation milestone.
- **Demand charges** are a possible future area of interest for battery modeling specifically — batteries can reduce peak-demand billing exposure via peak-shaving, a distinct value stream from time-of-use energy arbitrage.

---

## Scenario comparison (cross-cutting, not yet its own crate)

The real comparison problem is two-dimensional: multiple **scenarios** (baseline, +solar, +solar+battery, +heat pump, various combinations) each evaluated against multiple **offers**. Rough scale estimate: a year of 5-minute interval data is ~105,120 readings; with e.g. 8 scenarios × 500 offers, that's on the order of hundreds of millions of rate lookups for one customer's full analysis.

This is embarrassingly parallel across `(scenario, offer)` pairs — each pair's evaluation is independent of every other pair — which is the practical motivation for `rayon`-based parallelism (`par_iter()` over the outer product), and reinforces why `Tariff`'s point-query design matters: it keeps each individual evaluation a simple, pure, sequential sum, while the outer loop parallelizes cleanly.

---

## Open questions log

- Discount enum shape: percentage vs. flat-amount variants, plus which of {usage, total} each targets — not yet written as Rust types.
- Whether/how to expose currency and tax-inclusivity metadata on `Tariff` without leaking tax logic into the calculation engine.
- Full resolution of the "existing behind-the-meter equipment" reconstruction problem — parked, not blocking.