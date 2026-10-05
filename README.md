# open-energy

An open-source Rust platform for auditing and electrifying home and business energy use.

## Vision

The goal is to help residential and business users move away from fossil fuels (gas, diesel) —
fully or partially — by:

- **Auditing** a user's current energy profile from real smart meter data, as well as current
  gas, diesel, and other fossil fuel usage
- **Modeling electrification as a clean, carbon-free solution**: either by switching from
  gas/diesel to electric (solar panels, home batteries, replacing a diesel/petrol car with an
  electric one), or by efficiency upgrades to a better electric solution — mainly heat pumps for
  heating/cooling
- **Comparing efficiency** between electric solutions (e.g. before vs. after electrifying, or one
  electric option vs. another)
- **Switching to the best electricity offer**, via bill calculation from real usage data — usable
  independently, or before/after an electrification project

The project starts from the ground up: a solid, well-tested smart meter data parser and a
tariff-based bill calculator are the foundation everything else builds on.

## Why start with a parser

Australian electricity retailers and network operators exchange consumption data in the
[NEM12 format](https://aemo.com.au/) — a CSV-based standard defined by AEMO (Australian Energy
Market Operator). Accurately parsing this real-world data is the prerequisite for everything
downstream: billing calculations, offer comparison, and electrification modeling all depend on
having accurate, typed usage data to work from.

## Crates

- **`meter-parser`** — parses NEM12 CSV files into typed Rust structures (meter readings, units
  of measure, reading quality, import/export classification).
- **`bill-pricer`** — prices meter readings against a tariff: flat and time-of-use rates,
  weekday/weekend bands, daily supply charge, with validation that every day is fully covered
  by non-overlapping bands. Tariffs can be built in code or loaded from JSON.
- **`domain`** — shared types such as `HourMinute` and the localised pricing input.
- **`energy-cli`** — a command-line tool for running the parser and pricer against a file.

## Design notes

NEM12 timestamps use NEM market time (fixed UTC+10, no daylight saving). Readings are stored in
UTC and converted to the household's local time zone before tariff bands are applied, so
daylight-saving days (23 or 25 hours) are priced correctly.

## Usage

```bash
cargo run -p energy-cli -- parse --file data/raw/good_smart_meter.csv
```

Example output:

```
--- Running NEM12 Parser ---
Success! Parsed 786 days.
KwhImport: 8614.51300
```

## Development

After cloning, enable the repo's git hooks (blocks direct commits to `main`):

```bash
git config core.hooksPath scripts/hooks
```

Run tests across the workspace:

```bash
cargo test --workspace
```

Lint:

```bash
cargo clippy --workspace -- -W clippy::pedantic
```

Format:

## Contributing

Direct commits to main are blocked by the git hook described above; please work on a branch and open a pull request.

Before opening a pull request, run:

```bash
cargo fmt --all
cargo clippy --workspace -- -W clippy::pedantic
cargo test --workspace
```

## Status (Early stage)

**Working today**: NEM12 parsing, per-category totals, and bill calculation for flat,
time-of-use and weekday/weekend tariffs with DST-aware local time.

**In progress**: tiered rates, tax and currency handling, discounts, and pricing of export
(feed-in) energy. Not yet supported: seasonal tariffs and demand charges.

**Planned**, in rough order: electricity offer comparison/switching, then modeling of solar,
battery, heat pump, and EV solutions.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

## AI usage

I use AI assistants (Claude) for design discussions, code review and
drafting GitHub issues. The code, tests and design decisions are my own.