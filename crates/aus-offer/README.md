


## CDR plan converter

Maps Australian Consumer Data Right (CDR) electricity plans onto the
tariff model of the pricer crate. Every plan either converts or gets a
named reason why not.

### Usage


1. Download the plans (CDR public API):

```
cargo run --release -p energy-cli -- cdr plans --dir ./plans
```

2. Convert them:

```
cargo run --release -p energy-cli -- cdr offers --dir ./plans
```
### Results

Snapshot: 17,875 plans, 20+ retailers.

| Outcome | Count | Share |
|---|---|---|
| Converted | 16,519 | 92.4% |
| Unsupported | 1,356 | 7.6% |
| Failed | 0 | 0% |

Unsupported breakdown: demand charges (677), solar with multiple
options (460), solar multi flat/TOU (170), incomplete solar data (48),
other solar (1).

Performance: 0.14 s on 12 cores, 0.63 s single-threaded
(release build, warm cache).

### Definitions

- **Converted:** the plan maps to a valid tariff.
- **Unsupported:** valid plan the model cannot represent yet.
- **Failed:** malformed data.

### Conventions inferred from the data

- `endTime` may be inclusive or exclusive depending on the retailer.
  The converter infers it by checking whether an end time matches
  another band's start time.
- Multiple solar entries can mean tiers, alternatives or separate
  payers. Intent is inferred from the fields that differ.

### Limitations

- Conversion is not verified against known bills yet.
- Demand charges and overlapping solar options are not modelled.