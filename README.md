# tsl2584tsv

[![crates.io](https://img.shields.io/crates/v/tsl2584tsv.svg)](https://crates.io/crates/tsl2584tsv)
[![docs.rs](https://docs.rs/tsl2584tsv/badge.svg)](https://docs.rs/tsl2584tsv)
[![CI](https://github.com/mschoenlaub/tsl2584tsv/actions/workflows/ci.yml/badge.svg)](https://github.com/mschoenlaub/tsl2584tsv/actions/workflows/ci.yml)

`no_std` driver for the ams **TSL2584TSV**, an ambient light sensor (light-to-digital converter)
with a broadband and an infrared photodiode channel, controlled over I2C. It depends only on
`embedded-hal` 1.0, allocates nothing and has no `unsafe`.

- **The chip, not a board.** The bus address comes from the caller, one of the three the ADDR SEL
  pin selects. Turning the two channel counts into lux is left to the board: the coefficients
  depend on the glass or light pipe in front of the chip.
- **Typed configuration.** `Gain` is the four values of the Analog Register; `IntegrationTime` is
  1 to 255 cycles, never the zero that would select manual timing, and knows its nominal duration
  and its full-scale count.
- **Status read back, decoded.** The Control Register as `Status`, the identification registers as
  `DeviceId`, the two raw counts as `Channels` with a saturation check.
- **Every register and bit cited** to the datasheet, by figure and page.
- **Blocking and async.** The same driver over `embedded-hal-async` behind the feature `async`.

## Usage

The example identifies the chip, configures it for the longest integration time at the highest
gain, starts the ADCs and reads one pair of counts. The driver takes no delay provider: the 2 ms
the datasheet requires between powering on and enabling the ADCs, and the integration time before
the first valid read, are the caller's waits.

```rust
use embedded_hal::delay::DelayNs;
use embedded_hal::i2c::I2c;
use tsl2584tsv::{Channels, Error, Gain, IntegrationTime, POWER_ON_DELAY_MS, Tsl2584tsv, reg};

fn measure<I2C: I2c, D: DelayNs>(i2c: I2C, delay: &mut D) -> Result<Channels, Error<I2C::Error>> {
    // The ADDR SEL pin at GND gives address 0x29; floating gives 0x39, VDD gives 0x49.
    let mut als = Tsl2584tsv::new(i2c, reg::ADDRESS_ADDR_SEL_GND);
    als.check_id()?;

    // Power on, then configure: the datasheet wants the registers set before ADC_EN.
    als.power_on()?;
    als.set_integration_time(IntegrationTime::MAX)?; // 255 cycles, nominally 688.5 ms
    als.set_gain(Gain::X111)?;
    delay.delay_ms(POWER_ON_DELAY_MS);
    als.enable_adc()?;

    // One integration (plus a margin for the oscillator's tolerance) later, the counts are valid.
    delay.delay_us(IntegrationTime::MAX.nominal_micros() * 11 / 10);
    let counts = als.channels()?;
    if counts.saturated(IntegrationTime::MAX) {
        // Too bright for this gain and integration time: the counts say only "at least this much".
    }

    als.power_off()?;
    let _i2c = als.release();
    Ok(counts)
}
```

`status` reports whether a cycle has finished (`adc_valid`); `gain` and `integration_time` read
the configuration back. The interrupt pin is driven by a window on channel 0 (`set_thresholds`)
and a mode with a persistence (`set_interrupt`), and released with `clear_interrupt`. The crate
documentation has the full calling order, the interrupt's, how the chip is addressed on the bus,
and where the datasheet disagrees with itself.

Left out on purpose: manual integration (its special functions and timer registers are in `reg`
with their citations) and lux conversion, whose coefficients depend on the glass in front of the
chip.

## Ownership

The driver assumes nothing else writes to the chip while it holds it. Where another driver of
the chip is active, do not call the writing methods beside it; even a read moves the chip's
command register, and one that lands between the other driver's command write and its data read
can spoil one of its samples.

## Features

- `async`: `asynch::Tsl2584tsvAsync`, the same driver over `embedded-hal-async`. Off by default.

## Testing

`cargo test` runs one `embedded-hal-mock` test per write path, blocking and async, pinning the
bytes on the bus, plus unit tests of the saturation arithmetic, the interrupt encodings and the
error messages, and this README's example. On a chip, on a 2nd-generation Echo Dot from bare
metal: the identification reads (`check_id`) and the channel read (`channels`). The gain,
integration-time, status, interrupt and threshold reads, the interrupt writes and `power_off`
have run against the mock only.

## Related crates

- [`tsl2591`](https://crates.io/crates/tsl2591), [`tsl2591-eh-driver`](https://crates.io/crates/tsl2591-eh-driver)
  and [`tsl2591-rs`](https://crates.io/crates/tsl2591-rs) drive the ams TSL2591, a later sensor
  with a different register map (a command bit, interrupt thresholds with and without persistence)
  and a lux formula built in.
- [`tsl256x`](https://crates.io/crates/tsl256x) drives the older TSL2561 family over
  `embedded-hal` 0.2.
- [`opt300x`](https://crates.io/crates/opt300x), [`veml6030`](https://crates.io/crates/veml6030)
  and [`veml7700`](https://crates.io/crates/veml7700) are drivers for other vendors' ambient light
  sensors.

None of them covers the TSL2583 or TSL2584TSV.

## Sources

- ams **DS000352**, v1-08 (2017-Aug-28),
  [TSL2584TSV Light-to-Digital Device](https://look.ams-osram.com/m/a4a45ba84454eaa/original/TSL2584TSV-DS000352.pdf):
  every register address and bit, cited as "DS Figure N, p. N" (the datasheet numbers its tables
  as figures; the page is the one printed in the footer, one less than the PDF's page index).

## Minimum supported Rust version

Rust 1.87, tested in CI. Raising it is a minor version bump.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/mschoenlaub/tsl2584tsv/blob/main/LICENSE-APACHE) or
[MIT license](https://github.com/mschoenlaub/tsl2584tsv/blob/main/LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
