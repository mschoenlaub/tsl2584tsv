#![doc = include_str!("../README.md")]
//!
//! ## Using the driver
//!
//! From power-up every register is `00h`: powered off, manual timing, gain 1x (DS Figures 17, 18
//! and 23). To measure:
//!
//! 1. [`Tsl2584tsv::power_on`].
//! 2. [`Tsl2584tsv::set_integration_time`] and [`Tsl2584tsv::set_gain`]: "The device registers
//!    should be configured before ADC_EN is asserted" (DS Figure 17 note 2, p. 15).
//! 3. Wait until [`POWER_ON_DELAY_MS`] have passed since step 1. The driver takes no delay
//!    provider; the wait is the caller's.
//! 4. [`Tsl2584tsv::enable_adc`]. The chip now integrates continuously, one cycle after the other
//!    (DS "Detailed Description", p. 3).
//! 5. After one integration time, [`Tsl2584tsv::status`] reports `adc_valid`, and
//!    [`Tsl2584tsv::channels`] returns the counts.
//!
//! To change the gain or the integration time later, go back to [`Tsl2584tsv::power_on`], which
//! also stops the ADC, and repeat from step 2. [`Tsl2584tsv::power_off`] ends it.
//!
//! To use the interrupt pin, set the window and the mode in step 2 as well:
//! [`Tsl2584tsv::set_thresholds`], then [`Tsl2584tsv::set_interrupt`] with
//! [`InterruptMode::Level`] and a [`Persistence`]. The pin is open drain, active low, and compares
//! channel 0 only (DS p. 17). It stays asserted, and [`Status::adc_interrupt`] stays set, until
//! [`Tsl2584tsv::clear_interrupt`]. With [`InterruptConfig::stop_on_interrupt`] the ADC also
//! stops; resuming is [`Tsl2584tsv::power_on`], [`Tsl2584tsv::clear_interrupt`],
//! [`Tsl2584tsv::enable_adc`] (DS Figure 19, p. 17).
//!
//! With the `async` feature, `asynch::Tsl2584tsvAsync` is the same driver over
//! `embedded-hal-async`; it sends the same transfers.
//!
//! The ams TSL2540 has a different register model, and this crate does not cover it.
//!
//! ## How the chip is addressed
//!
//! Every access starts with a command byte that has bit 7 set and the register address in bits
//! 4:0 (DS Figure 16, p. 13 and 14; [`reg::command`]).
//!
//! - **Write of one register**: the command and the value in one write transfer, the SMBus
//!   "Write Byte" form. The datasheet's own description of the transfer is in a section v1-08
//!   lacks (page 12 refers to "the section on I²C protocols, above", and there is none); it
//!   names the SMBus protocols where it discusses byte pairs (Figure 22 note 1, p. 19; Figure 25
//!   note 1, p. 21), and this is the single-byte one.
//! - **Write of a threshold pair**: the auto-increment command for the low register, then the
//!   low and the high byte, the "Write Word protocol" the datasheet asks for so that a byte with
//!   bit 7 set is never taken for a command (Figure 22 note 1, p. 19).
//! - **Read of one register**: the command, a repeated start, one byte. The datasheet recommends
//!   "the Read Word protocol" for the byte pairs (Figure 25 note 1, p. 21) without describing it; in
//!   SMBus that name stands for a command byte, a repeated start and two data bytes, which is
//!   the combined form, and the datasheet's timing table does specify a repeated start
//!   (Figure 9, p. 8). One transfer instead of two also leaves no gap for another bus user.
//! - **Read of the channels**, and of the thresholds: the command with TRANSACTION `01`
//!   (auto-increment) for the first register, a repeated start, four bytes
//!   ([`reg::command_auto_increment`]). With TRANSACTION `00` every byte would be the same
//!   register again (DS Figure 16, p. 13).
//!
//! The two read forms have run on a chip (README, "Testing").
//!
//! ## Where the datasheet disagrees with itself
//!
//! - **Full scale.** Figure 8 (p. 7) gives 1024 counts per cycle less one, 37887 at 37 cycles,
//!   but the data registers are 16 bits wide, so from 65 cycles on the count cannot grow past
//!   65535; the same figure gives 65535 at 148 cycles. [`IntegrationTime::full_scale`] caps
//!   there.
//! - **The 111x gain.** Figure 23 (p. 20) names the highest gain 111x; Figure 8 (p. 7) gives the
//!   real scaling as typically 107 on channel 0 and 115 on channel 1, and board dependent.
//! - **Integration steps.** Figure 8 (p. 7) gives 256 as the maximum number of integration steps
//!   where Figure 18 (p. 16) allows 255, the value `0x00` being manual timing.
//! - **A dangling reference.** Figure 23 (p. 20) points to "Figure 24" for the gain table that is
//!   part of Figure 23 itself.
#![cfg_attr(not(test), no_std)]

use embedded_hal::i2c::I2c;

#[cfg(feature = "async")]
pub mod asynch;
pub mod reg;

/// How long to wait between [`Tsl2584tsv::power_on`] and [`Tsl2584tsv::enable_adc`], in
/// milliseconds: "After POWER is asserted, a 2-ms delay is required before asserting ADC_EN" (DS
/// Figure 17 note 1, p. 15).
pub const POWER_ON_DELAY_MS: u32 = 2;

/// The nominal length of one integration cycle in microseconds: the Timing Register counts "in
/// 2.7 ms increments" (DS Figure 18, p. 16).
///
/// The real step depends on the chip's oscillator: 2.73 ms typical, 2.58 ms to 2.90 ms (DS
/// Figure 8, "ADC integration time step size", and its note 4, p. 7).
pub const INTEGRATION_CYCLE_NOMINAL_US: u32 = 2_700;

/// The analog gain, field GAIN of the Analog Register (DS Figure 23, p. 20). The discriminant is
/// the field value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Gain {
    /// GAIN = `00`: 1x, the reset value.
    X1 = 0b00,
    /// GAIN = `01`: 8x. Measured against 1x it is 7 to 9 (DS Figure 8, p. 7).
    X8 = 0b01,
    /// GAIN = `10`: 16x. Measured against 1x it is 15 to 17 (DS Figure 8, p. 7).
    X16 = 0b10,
    /// GAIN = `11`: nominally 111x.
    ///
    /// The real scaling differs per channel and depends on the board: typically 107 on channel 0
    /// (97 to 115) and 115 on channel 1 (100 to 125), measured with the decoupling capacitor
    /// 25 mm from the VDD pin (DS Figure 8 and its note 5, p. 7; Figure 12, p. 9).
    X111 = 0b11,
}

impl Gain {
    /// Decodes the GAIN field from the Analog Register's value. The field is two bits wide and
    /// all four values are defined (DS Figure 23, p. 20), so this cannot fail.
    const fn from_bits(analog: u8) -> Self {
        match analog & reg::ANALOG_GAIN_MASK {
            0b00 => Gain::X1,
            0b01 => Gain::X8,
            0b10 => Gain::X16,
            _ => Gain::X111,
        }
    }
}

/// An integration time, as the number of 2.7 ms cycles the ADCs integrate for: 1 to 255 (DS
/// Figure 18, p. 16).
///
/// Longer integration gives more counts for the same light. Full scale grows with it, 1024
/// counts per cycle less one, until the 16-bit registers are full: 37887 at 37 cycles and 65535
/// at 148 cycles (DS Figure 8, "Full scale ADC count value", p. 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegrationTime {
    /// Never zero: the register value that would stand for it selects manual timing.
    cycles: u8,
}

impl IntegrationTime {
    /// One cycle, nominally 2.7 ms: the shortest (DS Figure 18, p. 16).
    pub const MIN: Self = Self { cycles: 1 };

    /// 255 cycles, nominally 688.5 ms: the longest (DS Figure 18, p. 16).
    pub const MAX: Self = Self { cycles: 255 };

    /// An integration time of `cycles` cycles, or `None` for zero: there is no zero-length
    /// integration, and the register value it would encode to is manual timing mode
    /// ([`reg::TIMING_MANUAL`]).
    pub const fn from_cycles(cycles: u8) -> Option<Self> {
        if cycles == 0 {
            None
        } else {
            Some(Self { cycles })
        }
    }

    /// The number of integration cycles, 1 to 255.
    pub const fn cycles(self) -> u8 {
        self.cycles
    }

    /// The nominal duration in microseconds, [`INTEGRATION_CYCLE_NOMINAL_US`] per cycle. A
    /// cycle can take 2.90 ms instead of 2.7 ms (DS Figure 8, p. 7), so allow about 8 % more
    /// than this before expecting a finished cycle.
    pub const fn nominal_micros(self) -> u32 {
        // A u8 widens to u32 without loss; `u32::from` is not callable in a const fn.
        self.cycles as u32 * INTEGRATION_CYCLE_NOMINAL_US
    }

    /// The ATIME value for the Timing Register: the two's complement of the cycle count (DS
    /// Figure 18, p. 16).
    const fn atime(self) -> u8 {
        self.cycles.wrapping_neg()
    }

    /// Decodes an ATIME value; `None` is manual timing mode (`0x00`).
    const fn from_atime(atime: u8) -> Option<Self> {
        Self::from_cycles(atime.wrapping_neg())
    }

    /// The full-scale count of a channel at this integration time: 1024 per cycle less one, at
    /// most 65535, the most the 16-bit data registers hold (DS Figure 8, "Full scale ADC count
    /// value": 37887 at 37 cycles, 65535 at 148). A count at full scale is saturated
    /// ([`Channels::saturated`]).
    pub const fn full_scale(self) -> u16 {
        let counts = self.cycles as u32 * 1024 - 1;
        if counts > u16::MAX as u32 {
            u16::MAX
        } else {
            counts as u16
        }
    }
}

// The encoding must reproduce the table in DS Figure 18 (p. 16), row by row. Checking it at compile
// time costs nothing and keeps a change to `atime` from passing unnoticed.
const _: () = {
    /// `atime` of a cycle count that is known not to be zero.
    const fn atime_of(cycles: u8) -> u8 {
        match IntegrationTime::from_cycles(cycles) {
            Some(time) => time.atime(),
            None => panic!("the datasheet's table has no zero row"),
        }
    }
    assert!(atime_of(1) == 0b1111_1111);
    assert!(atime_of(2) == 0b1111_1110);
    assert!(atime_of(19) == 0b1110_1101);
    assert!(atime_of(37) == 0b1101_1011);
    assert!(atime_of(74) == 0b1011_0110);
    assert!(atime_of(148) == 0b0110_1100);
    assert!(atime_of(255) == 0b0000_0001);
    assert!(IntegrationTime::from_atime(reg::TIMING_MANUAL).is_none());
    // 255 cycles are the datasheet's "maximum integration time", 688.5 ms.
    assert!(IntegrationTime::MAX.nominal_micros() == 688_500);
    // DS Figure 8's two full-scale rows.
    assert!(match IntegrationTime::from_cycles(37) {
        Some(time) => time.full_scale() == 37_887,
        None => false,
    });
    assert!(match IntegrationTime::from_cycles(148) {
        Some(time) => time.full_scale() == 65_535,
        None => false,
    });
    assert!(IntegrationTime::MAX.full_scale() == u16::MAX);
};

/// What the two identification registers hold (DS Figures 24 and 27, p. 20 and 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceId {
    /// The ID Register, `12h`: PARTNO in bits 7:4, REVNO in bits 3:0.
    pub id: u8,
    /// The ID2 Register, `1Eh`: bit 7 marks a TSL2584TSV, bits 6:0 are reserved.
    pub id2: u8,
}

impl DeviceId {
    /// PARTNO, the part number: `0b1001` on a TSL2584TSV (DS Figure 24, p. 20).
    pub const fn part_number(self) -> u8 {
        (self.id & reg::ID_PARTNO_MASK) >> reg::ID_PARTNO_SHIFT
    }

    /// REVNO, the silicon revision (DS Figure 24, p. 20). The datasheet names no value.
    pub const fn revision(self) -> u8 {
        self.id & reg::ID_REVNO_MASK
    }

    /// Whether both registers say TSL2584TSV: PARTNO is `1001b` (DS Figure 24, p. 20) and bit 7
    /// of ID2 is set (DS Figure 27, p. 22).
    ///
    /// The part number alone is not enough: the whole family shares it, and only the ID2 bit
    /// picks out the TSL2584TSV.
    pub const fn is_tsl2584tsv(self) -> bool {
        self.part_number() == reg::ID_PARTNO_TSL2584TSV && self.id2 & reg::ID2_TSL2584TSV != 0
    }
}

/// The interrupt mode, field INTR of the Interrupt Register (DS Figure 20, p. 18). The values
/// `10` and `11` are reserved; `11` "may be used to test interrupt connectivity in a system"
/// (Figure 20 note 1) and is not offered here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InterruptMode {
    /// INTR = `00`: the interrupt output is disabled, the reset value.
    Disabled = 0b00,
    /// INTR = `01`: a level interrupt, "generated whenever the last conversion results in a
    /// value outside of the programmed threshold window", that stays asserted until cleared with
    /// [`Tsl2584tsv::clear_interrupt`] (DS p. 17).
    Level = 0b01,
}

impl InterruptMode {
    /// Decodes the INTR field; `None` for the two reserved values.
    const fn from_bits(interrupt: u8) -> Option<Self> {
        match (interrupt & reg::INTERRUPT_INTR_MASK) >> reg::INTERRUPT_INTR_SHIFT {
            0b00 => Some(InterruptMode::Disabled),
            0b01 => Some(InterruptMode::Level),
            _ => None,
        }
    }
}

/// The interrupt persistence, field PERSIST of the Interrupt Register (DS Figure 21, p. 18): the
/// number of consecutive integration periods whose channel 0 count must lie outside the threshold
/// window before the interrupt asserts, 1 to 15, or [`EVERY_CYCLE`](Self::EVERY_CYCLE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Persistence {
    /// The field value, 0 to 15.
    value: u8,
}

impl Persistence {
    /// PERSIST = `0000`: "Every ADC cycle generates interrupt", whatever the counts. The reset
    /// value.
    pub const EVERY_CYCLE: Self = Self { value: 0 };

    /// PERSIST = `0001`: "Any value outside of threshold range", one period is enough.
    pub const ANY_OUT_OF_RANGE: Self = Self { value: 1 };

    /// The number of consecutive periods out of range it takes, 1 to 15 (`None` outside that).
    /// `periods(1)` is [`ANY_OUT_OF_RANGE`](Self::ANY_OUT_OF_RANGE); 0 is not a period count but
    /// [`EVERY_CYCLE`](Self::EVERY_CYCLE), which ignores the thresholds.
    pub const fn periods(periods: u8) -> Option<Self> {
        if periods >= 1 && periods <= 15 {
            Some(Self { value: periods })
        } else {
            None
        }
    }

    /// The period count this stands for; 0 for [`EVERY_CYCLE`](Self::EVERY_CYCLE).
    pub const fn into_periods(self) -> u8 {
        self.value
    }

    const fn from_bits(interrupt: u8) -> Self {
        Self {
            value: interrupt & reg::INTERRUPT_PERSIST_MASK,
        }
    }
}

/// The Interrupt Register, as written by [`Tsl2584tsv::set_interrupt`] and read back by
/// [`Tsl2584tsv::interrupt`] (DS Figure 19, p. 17).
///
/// The interrupt pin is open drain and active low, and "interrupts are based on the value of
/// Channel 0 only" (DS p. 17 and its note 1); the window is set with
/// [`Tsl2584tsv::set_thresholds`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterruptConfig {
    /// INTR: the mode.
    pub mode: InterruptMode,
    /// PERSIST: how many periods out of range assert the interrupt.
    pub persistence: Persistence,
    /// INTR_STOP: "ADC integration will stop once an interrupt is asserted". To resume,
    /// [`Tsl2584tsv::power_on`] (which clears ADC_EN), [`Tsl2584tsv::clear_interrupt`], then
    /// [`Tsl2584tsv::enable_adc`] again (DS Figure 19, p. 17).
    pub stop_on_interrupt: bool,
}

impl InterruptConfig {
    /// The reset value: the interrupt output disabled, every cycle, no stop.
    pub const DISABLED: Self = Self {
        mode: InterruptMode::Disabled,
        persistence: Persistence::EVERY_CYCLE,
        stop_on_interrupt: false,
    };

    /// The register value: INTR_STOP in bit 6, INTR in bits 5:4, PERSIST in bits 3:0, the
    /// reserved bit 7 "Write as 0" (DS Figure 19, p. 17).
    const fn to_bits(self) -> u8 {
        (if self.stop_on_interrupt {
            reg::INTERRUPT_INTR_STOP
        } else {
            0
        }) | ((self.mode as u8) << reg::INTERRUPT_INTR_SHIFT)
            | self.persistence.value
    }

    /// Decodes the register value; `None` when INTR holds a reserved value.
    const fn from_bits(interrupt: u8) -> Option<Self> {
        let Some(mode) = InterruptMode::from_bits(interrupt) else {
            return None;
        };
        Some(Self {
            mode,
            persistence: Persistence::from_bits(interrupt),
            stop_on_interrupt: interrupt & reg::INTERRUPT_INTR_STOP != 0,
        })
    }
}

/// The interrupt window on channel 0, the four Interrupt Threshold Registers (DS Figure 22,
/// p. 19): the interrupt asserts when channel 0 "crosses below or is equal to the low threshold"
/// or "crosses above the high threshold", subject to the persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// THLLOW and THLHIGH: the low threshold. Reset `0x0000`.
    pub low: u16,
    /// THHLOW and THHHIGH: the high threshold. Reset `0x0000`.
    pub high: u16,
}

impl Thresholds {
    /// Decodes the four registers in map order, each pair low byte first.
    const fn from_data(data: [u8; 4]) -> Self {
        Self {
            low: u16::from_le_bytes([data[0], data[1]]),
            high: u16::from_le_bytes([data[2], data[3]]),
        }
    }
}

/// The Control Register, decoded (DS Figure 17, p. 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    /// POWER: the chip is powered on.
    pub power: bool,
    /// ADC_EN: the ADC channels are enabled.
    pub adc_enabled: bool,
    /// ADC_VALID: "the ADC Channel has completed an integration cycle". The datasheet does not
    /// say what clears the bit.
    pub adc_valid: bool,
    /// ADC_INTR: "the device is asserting an interrupt". It stays set until
    /// [`Tsl2584tsv::clear_interrupt`] (DS p. 17).
    pub adc_interrupt: bool,
}

impl Status {
    /// Decodes the register value.
    const fn from_bits(control: u8) -> Self {
        Self {
            power: control & reg::CONTROL_POWER != 0,
            adc_enabled: control & reg::CONTROL_ADC_EN != 0,
            adc_valid: control & reg::CONTROL_ADC_VALID != 0,
            adc_interrupt: control & reg::CONTROL_ADC_INTR != 0,
        }
    }
}

/// The raw counts of the two ADC channels from one read of the data registers (DS Figure 25,
/// p. 21).
///
/// The chip has a broadband photodiode (visible plus infrared), an infrared-responding one and a
/// photopic filter (DS "General Description", p. 1). The response of each channel over wavelength
/// is DS Figure 11 (p. 9); under white light channel 1 is typically 0.072 of channel 0 (DS
/// Figure 8, p. 7). Turning the pair into lux is not part of this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    /// ADC channel 0, DATA0HIGH:DATA0LOW. The interrupt thresholds compare against this one (DS
    /// Figure 19 note 1, p. 17).
    pub ch0: u16,
    /// ADC channel 1, DATA1HIGH:DATA1LOW.
    pub ch1: u16,
}

impl Channels {
    /// Decodes the four data registers as read from DATA0LOW on: two little-endian counts (DS
    /// Figure 25, p. 21). Shared by the blocking and the async driver.
    const fn from_data(data: [u8; 4]) -> Self {
        Self {
            ch0: u16::from_le_bytes([data[0], data[1]]),
            ch1: u16::from_le_bytes([data[2], data[3]]),
        }
    }

    /// Whether either channel is at the full scale of `time` ([`IntegrationTime::full_scale`]):
    /// the light was too strong for the gain and integration time, and the count says only
    /// "at least this much". Above 64 cycles the full scale is the registers' 65535.
    pub const fn saturated(self, time: IntegrationTime) -> bool {
        let full = time.full_scale();
        self.ch0 >= full || self.ch1 >= full
    }
}

/// Errors from the sensor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error<E> {
    /// The I2C transaction failed.
    Bus(E),
    /// The identification registers were read, and they are not a TSL2584TSV's. Carries what was
    /// read.
    UnexpectedId(DeviceId),
    /// The Interrupt Register was read, and its INTR field holds one of the two reserved values
    /// (DS Figure 20, p. 18), which this driver never writes. Carries the register value.
    ReservedInterruptMode(u8),
}

/// Names the failure. A bus error is shown with its `Debug` form, the one representation every
/// `embedded-hal` bus error has.
impl<E: core::fmt::Debug> core::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Bus(error) => write!(f, "TSL2584TSV I2C error: {error:?}"),
            Self::UnexpectedId(id) => write!(
                f,
                "not a TSL2584TSV: ID register {:#04x}, ID2 register {:#04x}",
                id.id, id.id2
            ),
            Self::ReservedInterruptMode(value) => write!(
                f,
                "TSL2584TSV Interrupt Register {value:#04x} holds a reserved INTR value"
            ),
        }
    }
}

impl<E: core::fmt::Debug> core::error::Error for Error<E> {}

/// A TSL2584TSV on an `embedded-hal` I2C bus.
pub struct Tsl2584tsv<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C: I2c> Tsl2584tsv<I2C> {
    /// Wraps the bus; talks to nothing until a method is called. `address` is set by the chip's
    /// ADDR SEL pin ([`reg::ADDRESS_ADDR_SEL_GND`] and its neighbours).
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Gives the bus back.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// Reads the ID and ID2 registers without judging them.
    ///
    /// The datasheet does not say whether they can be read while the chip is powered off. The
    /// registers can be read before the first write of POWER, which
    /// suggests that they can.
    pub fn device_id(&mut self) -> Result<DeviceId, Error<I2C::Error>> {
        let id = self.read_register(reg::ID)?;
        let id2 = self.read_register(reg::ID2)?;
        Ok(DeviceId { id, id2 })
    }

    /// Reads the identification registers and fails with [`Error::UnexpectedId`] unless they are
    /// a TSL2584TSV's ([`DeviceId::is_tsl2584tsv`]). Returns what was read, for the revision.
    pub fn check_id(&mut self) -> Result<DeviceId, Error<I2C::Error>> {
        let id = self.device_id()?;
        if id.is_tsl2584tsv() {
            Ok(id)
        } else {
            Err(Error::UnexpectedId(id))
        }
    }

    /// Reads and decodes the Control Register.
    pub fn status(&mut self) -> Result<Status, Error<I2C::Error>> {
        Ok(Status::from_bits(self.read_register(reg::CONTROL)?))
    }

    /// Powers the chip on with the ADCs disabled: writes POWER alone to the Control Register.
    ///
    /// This is the state in which to set the gain and the integration time, and the state to
    /// wait [`POWER_ON_DELAY_MS`] in before [`enable_adc`](Self::enable_adc) (DS Figure 17
    /// notes 1 and 2, p. 15). Called while the ADCs run, it stops them and leaves the chip
    /// powered.
    pub fn power_on(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, reg::CONTROL_POWER)
    }

    /// Starts the ADCs: writes POWER and ADC_EN to the Control Register. The chip then
    /// integrates one cycle after the other with the gain and integration time it has.
    ///
    /// Call [`power_on`](Self::power_on) first and let [`POWER_ON_DELAY_MS`] pass: "ADC_EN and
    /// POWER must be asserted before the ADC changes will operate correctly. After POWER is
    /// asserted, a 2-ms delay is required before asserting ADC_EN" (DS Figure 17 note 1, p. 15).
    pub fn enable_adc(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, reg::CONTROL_POWER | reg::CONTROL_ADC_EN)
    }

    /// Powers the chip off: writes `00h` to the Control Register, clearing ADC_EN and POWER (DS
    /// Figure 17, p. 15).
    ///
    /// The datasheet does not say whether the other registers keep their values while the chip
    /// is off, so set the gain and the integration time again after the next
    /// [`power_on`](Self::power_on).
    pub fn power_off(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, 0x00)
    }

    /// Reads the gain from the Analog Register.
    pub fn gain(&mut self) -> Result<Gain, Error<I2C::Error>> {
        Ok(Gain::from_bits(self.read_register(reg::ANALOG)?))
    }

    /// Sets the gain: writes the GAIN field to the Analog Register with the reserved bits 7:2
    /// as 0, as DS Figure 23 (p. 20) requires.
    ///
    /// Do this while the ADCs are disabled, between [`power_on`](Self::power_on) and
    /// [`enable_adc`](Self::enable_adc) (DS Figure 17 note 2, p. 15).
    pub fn set_gain(&mut self, gain: Gain) -> Result<(), Error<I2C::Error>> {
        // The discriminant is the GAIN field value (`#[repr(u8)]`), which sits at bit 0.
        self.write_register(reg::ANALOG, gain as u8)
    }

    /// Reads the integration time from the Timing Register. `None` means the register holds
    /// `00h`, manual timing mode, which is its reset value and which this driver does not
    /// operate.
    pub fn integration_time(&mut self) -> Result<Option<IntegrationTime>, Error<I2C::Error>> {
        Ok(IntegrationTime::from_atime(
            self.read_register(reg::TIMING)?,
        ))
    }

    /// Sets the integration time: writes the two's complement of the cycle count to the Timing
    /// Register (DS Figure 18, p. 16).
    ///
    /// Do this while the ADCs are disabled, between [`power_on`](Self::power_on) and
    /// [`enable_adc`](Self::enable_adc) (DS Figure 17 note 2, p. 15).
    pub fn set_integration_time(&mut self, time: IntegrationTime) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::TIMING, time.atime())
    }

    /// Reads the raw counts of both channels in one transfer.
    ///
    /// The counts are those of the last finished integration cycle (DS "Detailed Description",
    /// p. 3). The data registers default to `00h` at power-up (DS p. 21), and
    /// [`status`](Self::status) tells through `adc_valid` whether a cycle has finished.
    ///
    /// The four data registers are read with address auto-increment, low byte before high byte
    /// for each channel. That order is required: reading a low byte latches the channel's high
    /// byte into a shadow register, so the pair belongs to one cycle even if a cycle ends in
    /// between (DS p. 21). Nothing in the datasheet latches channel 1 together with channel 0;
    /// a cycle that ends between the second and third byte gives the two channels from
    /// neighbouring cycles.
    pub fn channels(&mut self) -> Result<Channels, Error<I2C::Error>> {
        let mut data = [0u8; 4];
        self.i2c
            .write_read(
                self.address,
                &[reg::command_auto_increment(reg::DATA0LOW)],
                &mut data,
            )
            .map_err(Error::Bus)?;
        Ok(Channels::from_data(data))
    }

    /// Reads the Interrupt Register.
    pub fn interrupt(&mut self) -> Result<InterruptConfig, Error<I2C::Error>> {
        let value = self.read_register(reg::INTERRUPT)?;
        InterruptConfig::from_bits(value).ok_or(Error::ReservedInterruptMode(value))
    }

    /// Writes the Interrupt Register: the mode, the persistence and INTR_STOP (DS Figure 19,
    /// p. 17). Set the window with [`set_thresholds`](Self::set_thresholds) first; like the other
    /// registers it belongs before [`enable_adc`](Self::enable_adc) (DS Figure 17 note 2, p. 15).
    pub fn set_interrupt(&mut self, config: InterruptConfig) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::INTERRUPT, config.to_bits())
    }

    /// Reads the four threshold registers in one auto-increment transfer.
    pub fn thresholds(&mut self) -> Result<Thresholds, Error<I2C::Error>> {
        let mut data = [0u8; 4];
        self.i2c
            .write_read(
                self.address,
                &[reg::command_auto_increment(reg::THLLOW)],
                &mut data,
            )
            .map_err(Error::Bus)?;
        Ok(Thresholds::from_data(data))
    }

    /// Writes the interrupt window: each 16-bit threshold as one transfer of its two registers,
    /// low byte then high byte with address auto-increment, which is the "Write Word protocol"
    /// the datasheet asks for so that neither byte can be taken for a command (DS Figure 22
    /// note 1, p. 19).
    pub fn set_thresholds(&mut self, thresholds: Thresholds) -> Result<(), Error<I2C::Error>> {
        self.write_word(reg::THLLOW, thresholds.low)?;
        self.write_word(reg::THHLOW, thresholds.high)
    }

    /// Clears a pending interrupt and releases the pin: the one-byte "Interrupt clear" special
    /// function (DS Figure 16, p. 14). Also clears ADC_INTR in [`Status`].
    pub fn clear_interrupt(&mut self) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(
                self.address,
                &[reg::command_special(reg::SPECIAL_INTERRUPT_CLEAR)],
            )
            .map_err(Error::Bus)
    }

    /// Reads one register: the command byte, a repeated start and one data byte (crate docs,
    /// "How the chip is addressed").
    fn read_register(&mut self, register: u8) -> Result<u8, Error<I2C::Error>> {
        let mut buf = [0u8];
        self.i2c
            .write_read(self.address, &[reg::command(register)], &mut buf)
            .map_err(Error::Bus)?;
        Ok(buf[0])
    }

    /// Writes one register: the command byte and the value in one transfer (crate docs, "How the
    /// chip is addressed").
    fn write_register(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.address, &[reg::command(register), value])
            .map_err(Error::Bus)
    }

    /// Writes a register pair, `register` and the one above it, with the two bytes of `value`
    /// in one auto-increment transfer, low byte first (DS Figure 22 note 1, p. 19).
    fn write_word(&mut self, register: u8, value: u16) -> Result<(), Error<I2C::Error>> {
        let [low, high] = value.to_le_bytes();
        self.i2c
            .write(
                self.address,
                &[reg::command_auto_increment(register), low, high],
            )
            .map_err(Error::Bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    /// The ADDR SEL = GND address; any of the three would do for these tests.
    const ADDRESS: u8 = reg::ADDRESS_ADDR_SEL_GND;

    // These tests pin the register write sequences only, one per write path, plus the pure
    // saturation arithmetic and the error messages. The read paths are checked on a chip (README,
    // "Testing").

    #[test]
    fn power_on_writes_power_alone() {
        // 0x80 is the command for CONTROL; 0x01 is POWER without ADC_EN.
        let expectations = [Transaction::write(ADDRESS, vec![0x80, 0x01])];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.power_on(), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn enable_adc_writes_power_and_adc_en() {
        // 0x03 is POWER and ADC_EN: the running state.
        let expectations = [Transaction::write(ADDRESS, vec![0x80, 0x03])];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.enable_adc(), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn power_off_clears_the_control_register() {
        let expectations = [Transaction::write(ADDRESS, vec![0x80, 0x00])];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.power_off(), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn set_gain_writes_the_gain_field_with_reserved_bits_clear() {
        // 0x87 is the command for ANALOG. 0x03 is 111x; 0x00 is 1x, the reset value.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x87, 0x03]),
            Transaction::write(ADDRESS, vec![0x87, 0x02]),
            Transaction::write(ADDRESS, vec![0x87, 0x01]),
            Transaction::write(ADDRESS, vec![0x87, 0x00]),
        ];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.set_gain(Gain::X111), Ok(()));
        assert_eq!(sensor.set_gain(Gain::X16), Ok(()));
        assert_eq!(sensor.set_gain(Gain::X8), Ok(()));
        assert_eq!(sensor.set_gain(Gain::X1), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn set_integration_time_writes_the_twos_complement_of_the_cycles() {
        // 0x81 is the command for TIMING. 0x01 is 255 cycles; 0xdb is the datasheet's 37
        // cycles, 99.9 ms; 0xff is one cycle.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x81, 0x01]),
            Transaction::write(ADDRESS, vec![0x81, 0xdb]),
            Transaction::write(ADDRESS, vec![0x81, 0xff]),
        ];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.set_integration_time(IntegrationTime::MAX), Ok(()));
        let cycles_37 = IntegrationTime::from_cycles(37).expect("37 is not zero");
        assert_eq!(sensor.set_integration_time(cycles_37), Ok(()));
        assert_eq!(sensor.set_integration_time(IntegrationTime::MIN), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn set_interrupt_writes_mode_persistence_and_stop_bit() {
        // 0x82 is the command for INTERRUPT. 0x1a: level interrupt (INTR = 01) after 10 periods
        // out of range; 0x51: INTR_STOP, level, any value out of range; 0x00: the reset value.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x82, 0x1a]),
            Transaction::write(ADDRESS, vec![0x82, 0x51]),
            Transaction::write(ADDRESS, vec![0x82, 0x00]),
        ];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        let ten = Persistence::periods(10).expect("10 is 1 to 15");
        let level_after_ten = InterruptConfig {
            mode: InterruptMode::Level,
            persistence: ten,
            stop_on_interrupt: false,
        };
        assert_eq!(sensor.set_interrupt(level_after_ten), Ok(()));
        let stop_on_any = InterruptConfig {
            mode: InterruptMode::Level,
            persistence: Persistence::ANY_OUT_OF_RANGE,
            stop_on_interrupt: true,
        };
        assert_eq!(sensor.set_interrupt(stop_on_any), Ok(()));
        assert_eq!(sensor.set_interrupt(InterruptConfig::DISABLED), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn set_thresholds_writes_each_pair_as_one_word() {
        // 0xa3 and 0xa5 are the auto-increment commands for THLLOW and THHLOW: each 16-bit
        // threshold goes low byte then high byte in one transfer.
        let expectations = [
            Transaction::write(ADDRESS, vec![0xa3, 0x03, 0x00]),
            Transaction::write(ADDRESS, vec![0xa5, 0xff, 0xff]),
        ];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        let window = Thresholds {
            low: 3,
            high: 0xffff,
        };
        assert_eq!(sensor.set_thresholds(window), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn clear_interrupt_sends_the_special_function_byte() {
        // 0xe1: CMD, TRANSACTION `11` (special function), function `00001` (interrupt clear).
        let expectations = [Transaction::write(ADDRESS, vec![0xe1])];
        let mut sensor = Tsl2584tsv::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(sensor.clear_interrupt(), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn persistence_is_zero_to_fifteen_periods() {
        assert_eq!(Persistence::periods(0), None);
        assert_eq!(Persistence::periods(16), None);
        assert_eq!(Persistence::periods(1), Some(Persistence::ANY_OUT_OF_RANGE));
        assert_eq!(
            Persistence::periods(15).map(Persistence::into_periods),
            Some(15)
        );
        assert_eq!(Persistence::EVERY_CYCLE.into_periods(), 0);
    }

    #[test]
    fn interrupt_config_decodes_the_register() {
        let decoded = InterruptConfig::from_bits(0x51).expect("INTR = 01 is defined");
        assert_eq!(decoded.mode, InterruptMode::Level);
        assert_eq!(decoded.persistence, Persistence::ANY_OUT_OF_RANGE);
        assert!(decoded.stop_on_interrupt);
        // INTR = 10 is reserved: decoded as `None`, never as a mode the chip does not define.
        assert_eq!(InterruptConfig::from_bits(0x20), None);
        assert_eq!(
            InterruptConfig::from_bits(0x00),
            Some(InterruptConfig::DISABLED)
        );
    }

    #[test]
    fn thresholds_decode_little_endian_pairs() {
        assert_eq!(
            Thresholds::from_data([0x03, 0x00, 0xff, 0xff]),
            Thresholds {
                low: 3,
                high: 0xffff
            }
        );
    }

    #[test]
    fn a_channel_at_full_scale_is_saturated() {
        let t74 = IntegrationTime::from_cycles(74).expect("74 is not zero");
        let t37 = IntegrationTime::from_cycles(37).expect("37 is not zero");
        // 74 cycles reach the registers' limit (75775 > 65535); 37 cycles stop at 37887.
        assert_eq!(t74.full_scale(), u16::MAX);
        assert_eq!(IntegrationTime::MIN.full_scale(), 1023);
        let lit = Channels {
            ch0: 37_886,
            ch1: 3_000,
        };
        assert!(!lit.saturated(t37));
        assert!(
            Channels {
                ch0: 37_887,
                ch1: 3_000
            }
            .saturated(t37)
        );
        assert!(
            Channels {
                ch0: 100,
                ch1: 37_887
            }
            .saturated(t37)
        );
        assert!(!lit.saturated(t74));
        assert!(
            Channels {
                ch0: u16::MAX,
                ch1: 0
            }
            .saturated(IntegrationTime::MAX)
        );
    }

    #[test]
    fn errors_name_the_failure() {
        extern crate std;
        use std::string::ToString;
        let wrong = Error::<()>::UnexpectedId(DeviceId {
            id: 0x00,
            id2: 0x80,
        });
        assert_eq!(
            wrong.to_string(),
            "not a TSL2584TSV: ID register 0x00, ID2 register 0x80"
        );
        assert_eq!(Error::Bus(()).to_string(), "TSL2584TSV I2C error: ()");
        assert_eq!(
            Error::<()>::ReservedInterruptMode(0x20).to_string(),
            "TSL2584TSV Interrupt Register 0x20 holds a reserved INTR value"
        );
    }
}
