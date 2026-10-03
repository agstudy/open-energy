# energy-cli

Command-line tool for working with NEM12 smart meter data: parsing raw
files and pricing electricity usage against a tariff.

Part of the [open-energy](../) workspace — built on `meter-parser`,
`bill-pricer`, and `domain`.

## Install / run

From the workspace root:

```bash
cargo run -p energy-cli -- <command> [options]
```

Or build a release binary:

```bash
cargo build -p energy-cli --release
./target/release/energy-cli <command> [options]
```

## Commands

### `parse`

Parse a NEM12 CSV file and print a summary.

```bash
energy-cli parse --file data/raw/good_smart_meter.csv
energy-cli parse --file data/raw/good_smart_meter.csv --verbose
```

| Flag              | Short | Description                                 |
|-------------------|-------|----------------------------------------------|
| `--file <PATH>`   | `-f`  | Path to the NEM12 CSV file (required)         |
| `--verbose`       | `-v`  | Print each parsed interval result in detail   |

### `price`

Parse a NEM12 file and compute the total bill against a tariff. The
tariff is chosen via a nested subcommand: `flat` or `tou`.

```bash
energy-cli price --file <PATH> --tz <TIMEZONE> <flat|tou|from-file> [tariff options]
```

| Flag            | Short | Description                        |
|------------------ |-------|-------------------------------------|
| `--file <PATH>`   | `-f`  | Path to the NEM12 CSV file (required) |
|`--tz <TIMEZONE>`	|`-t`	 |Timezone for pricing (e.g. Australia/Sydney) (required)|

#### `flat` — single flat rate

```bash
energy-cli price --file data/raw/good_smart_meter.csv flat \
  --rate 0.20 \
  --supply 1.00
```

| Flag             | Description                          |
|-------------------|----------------------------------------|
| `--rate <DECIMAL>`   | Usage rate, $/kWh                    |
| `--supply <DECIMAL>` | Daily supply charge, $/day           |

#### `tou` — time-of-use (peak / off-peak)

```bash
energy-cli price --file data/raw/good_smart_meter.csv tou \
  --peak 0.30 --start-peak 14:00 \
  --off-peak 0.15 --start-off-peak 22:00 \
  --supply 0.20
```

| Flag                     | Description                                  |
|---------------------------|-------------------------------------------------|
| `--peak <DECIMAL>`          | Peak usage rate, $/kWh                        |
| `--start-peak <HH:MM>`      | Time the peak window begins (24h, e.g. `14:00`) |
| `--off-peak <DECIMAL>`      | Off-peak usage rate, $/kWh                    |
| `--start-off-peak <HH:MM>`  | Time the off-peak window begins (24h)          |
| `--supply <DECIMAL>`        | Daily supply charge, $/day                    |

Times must be in `HH:MM` 24-hour format (`00:00`–`23:59`); an invalid
format or out-of-range hour/minute is rejected with an error message
rather than silently accepted.


##  `from-file` — load a tariff from a JSON file
```bash
energy-cli price --file data/raw/good_smart_meter.csv --tz Australia/Sydney \
  from-file --tariff-file tariffs/my_tariff.json
  ``
|Flag	|Description|
|`--tariff-file` |<PATH>	Path to a JSON tariff definition (required)|

## Status

This is an early (v0) CLI built alongside the open-energy workspace.
Known limitations:

- Only flat and two-band time-of-use tariffs are supported so far —
  no tiered rates, discounts, or eligibility constraints yet.
- Tariffs are specified entirely via CLI flags; no support yet for
  loading a tariff from a file.
- No demand-charge (peak kW) support yet.
- The JSON tariff format for `from-file` is not yet documented or
stabilised — it maps directly to the TariffRaw type in
bill-pricer.

## Examples

```bash
# Inspect what was parsed
energy-cli parse --file data/raw/good_smart_meter.csv --verbose

# Price it against a flat rate
energy-cli price --file data/raw/good_smart_meter.csv --tz Australia/Sydney \
  flat --rate 0.20 --supply 1.00

# Price it against time-of-use
energy-cli price --file data/raw/good_smart_meter.csv --tz Australia/Sydney \
  tou --peak 0.30 --start-peak 14:00 \
  --off-peak 0.15 --start-off-peak 22:00 \
  --supply 0.20

# Price it against a tariff loaded from a file
energy-cli price --file data/raw/good_smart_meter.csv --tz Australia/Sydney \
  from-file --tariff-file tariffs/my_tariff.json

```

Run `energy-cli <command> --help` or `energy-cli price <flat|tou|from-file> --help`
for the full generated usage for any command.