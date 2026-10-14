# Changelog

All notable changes to this crate are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-10-14

First release.

- The register map (`reg`): every register, bit field, special function and the three bus
  addresses, each cited to the datasheet, with the command-byte helpers for single-register and
  auto-increment access.
- Identification: `device_id` and `check_id`, which read the ID and ID2 registers and recognise a
  TSL2584TSV by both.
- Power and ADC control: `power_on`, `enable_adc`, `power_off`, and the Control Register decoded
  as `Status`; `POWER_ON_DELAY_MS` for the wait the datasheet requires between the two.
- Configuration: `Gain` and `IntegrationTime` (1 to 255 cycles, with `nominal_micros` and
  `full_scale`), written and read back.
- The two raw channel counts in one auto-increment transfer (`Channels`), with `saturated`.
- The interrupt: `Thresholds` on channel 0, `InterruptConfig` (`InterruptMode`, `Persistence`,
  stop on interrupt) written and read back, and `clear_interrupt`.
- A typed error, `Error<E>`, with `Display` and `core::error::Error`.
- The same driver over `embedded-hal-async`, `asynch::Tsl2584tsvAsync`, behind the feature
  `async`.

[Unreleased]: https://github.com/mschoenlaub/tsl2584tsv/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mschoenlaub/tsl2584tsv/releases/tag/v0.1.0
