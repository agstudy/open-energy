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

The project starts from the ground up: a solid, well-tested smart meter data parser is the
foundation everything else builds on.

## Why start with a parser

Australian electricity retailers and network operators exchange consumption data in the
[NEM12 format](https://aemo.com.au/) — a CSV-based standard defined by AEMO (Australian Energy
Market Operator). Accurately parsing this real-world data is the prerequisite for everything
downstream: billing calculations, offer comparison, and electrification modeling all depend on
having accurate, typed usage data to work from.

## Crates

- **`meter-parser`** — parses NEM12 CSV files into typed Rust structures (meter readings, units
  of measure, reading quality, import/export classification).
- **`energy-cli`** — a command-line tool for running the parser against a file and printing a
  summary.

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

Run tests across the workspace:

```bash
cargo test --workspace
```

Lint:

```bash
cargo clippy --workspace -- -W clippy::pedantic
```

Format:

```bash
cargo fmt --all
```

## Status

Early stage. Currently supports parsing NEM12 files and computing basic per-category totals
(import/export energy). Planned, in rough order: tariff-based billing calculation, electricity
offer comparison/switching, and modeling of solar, battery, heat pump, and EV solutions.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
