//! Register map of the TSL2584TSV, read from the manufacturer's datasheet and cross-checked against
//! the datasheet.
//!
//! "DS" is ams datasheet **DS000352, v1-08 (2017-Aug-28)**, "TSL2584TSV Light-to-Digital Device"
//! (`https://look.ams-osram.com/m/a4a45ba84454eaa/original/TSL2584TSV-DS000352.pdf`). Citations
//! name the figure (the datasheet numbers its tables as figures) and the page printed in the page's
//! footer, which is one less than the PDF's page index because the file starts with a cover sheet.
//!

/// 7-bit bus address with the ADDR SEL pin at GND: `0101001` (DS Figure 14, p. 11).
pub const ADDRESS_ADDR_SEL_GND: u8 = 0x29;

/// 7-bit bus address with the ADDR SEL pin floating: `0111001` (DS Figure 14, p. 11).
pub const ADDRESS_ADDR_SEL_FLOAT: u8 = 0x39;

/// 7-bit bus address with the ADDR SEL pin at VDD: `1001001` (DS Figure 14, p. 11).
pub const ADDRESS_ADDR_SEL_VDD: u8 = 0x49;

/// CMD, bit 7 of the Command Register: "Must write as 1 when addressing COMMAND register" (DS
/// Figure 16, p. 13).
///
/// The Command Register has no address of its own (DS Figure 15, p. 12, lists it with "−−"): the
/// first byte of a write transfer is the command, and it selects the register that the following
/// data bytes or the following read go to.
pub const COMMAND_CMD: u8 = 0b1000_0000;

/// TRANSACTION, bits 6:5 of the Command Register: the kind of transfer that follows (DS
/// Figure 16, p. 13). The value `10` is "Reserved - Do not use".
pub const COMMAND_TRANSACTION_MASK: u8 = 0b0110_0000;

/// TRANSACTION = `00`, "Repeated byte protocol transaction": every data access goes to the same
/// register (DS Figure 16, p. 13): the form for a single-register read or write.
pub const COMMAND_TRANSACTION_REPEATED_BYTE: u8 = 0b0000_0000;

/// TRANSACTION = `01`, "Auto - increment protocol transaction": "will provide an auto-increment
/// function to read successive register bytes" (DS Figure 16, p. 13), and is also the form of the
/// "Write Word protocol" for a register pair (Figure 22 note 1, p. 19).
pub const COMMAND_TRANSACTION_AUTO_INCREMENT: u8 = 0b0010_0000;

/// TRANSACTION = `11`, "Special function": the ADDRESS field then holds one of the `SPECIAL_*`
/// values instead of a register address (DS Figure 16, p. 13 and 14).
pub const COMMAND_TRANSACTION_SPECIAL_FUNCTION: u8 = 0b0110_0000;

/// ADDRESS, bits 4:0 of the Command Register: the register address from DS Figure 15, or the
/// special function when TRANSACTION is `11` (DS Figure 16, p. 14).
pub const COMMAND_ADDRESS_MASK: u8 = 0b0001_1111;

/// Special function `00001`, "Interrupt clear": "Clear any pending interrupt and is a
/// write-once-to-clear bit" (DS Figure 16, p. 14): the single byte `0xE1`.
pub const SPECIAL_INTERRUPT_CLEAR: u8 = 0b0_0001;

/// Special function `00010`, "Stop manual integration", for use while the Timing Register holds
/// `00h` (DS Figure 16, p. 14).
pub const SPECIAL_STOP_MANUAL_INTEGRATION: u8 = 0b0_0010;

/// Special function `00011`, "Start manual integration", for use while the Timing Register holds
/// `00h` (DS Figure 16, p. 14).
pub const SPECIAL_START_MANUAL_INTEGRATION: u8 = 0b0_0011;

/// CONTROL, `00h`, read/write, reset `00h`: "Control of basic functions" (DS Figure 15, p. 12;
/// Figure 17, p. 15).
pub const CONTROL: u8 = 0x00;

/// POWER, bit 0 of [`CONTROL`]: "Writing a 1 powers on the device, and writing a 0 turns it off"
/// (DS Figure 17, p. 15).
pub const CONTROL_POWER: u8 = 0b0000_0001;

/// ADC_EN, bit 1 of [`CONTROL`]: "Writing a 1 activates the ADC Channels, and writing a 0
/// disables the ADCs" (DS Figure 17, p. 15).
///
/// It only works with POWER set, and it may be set 2 ms after POWER at the earliest (DS
/// Figure 17, note 1).
pub const CONTROL_ADC_EN: u8 = 0b0000_0010;

/// ADC_VALID, bit 4 of [`CONTROL`], read only: "Indicates that the ADC Channel has completed an
/// integration cycle" (DS Figure 17, p. 15).
///
pub const CONTROL_ADC_VALID: u8 = 0b0001_0000;

/// ADC_INTR, bit 5 of [`CONTROL`], read only: "Indicates that the device is asserting an
/// interrupt" (DS Figure 17, p. 15).
pub const CONTROL_ADC_INTR: u8 = 0b0010_0000;

// Bits 7:6 and 3:2 of CONTROL are "Reserved. Write as 0" (DS Figure 17, p. 15). POWER and ADC_EN
// are therefore the only bits a write can set, and the driver writes the whole register instead of
// reading it first.

/// TIMING, `01h`, read/write, reset `00h`: the integration time as the field ATIME, all eight
/// bits (DS Figure 15, p. 12; Figure 18, p. 16).
///
/// ATIME is the two's complement of the number of 2.7 ms integration cycles: `0xFF` is one
/// cycle, `0x01` is 255 cycles. `0x00`, the reset value, selects manual timing instead, where an
/// integration is started and stopped with the special functions.
pub const TIMING: u8 = 0x01;

/// The ATIME value that selects manual timing mode, and the register's reset value (DS
/// Figure 18, p. 16).
pub const TIMING_MANUAL: u8 = 0x00;

/// INTERRUPT, `02h`, read/write, reset `00h`: INTR_STOP in bit 6, INTR in bits 5:4, PERSIST in
/// bits 3:0 (DS Figure 15, p. 12; Figure 19, p. 17).
pub const INTERRUPT: u8 = 0x02;

/// INTR_STOP, bit 6 of [`INTERRUPT`]: "Stop ADC Integration on Interrupt. When high, ADC
/// integration will stop once an interrupt is asserted" (DS Figure 19, p. 17).
pub const INTERRUPT_INTR_STOP: u8 = 0b0100_0000;

/// INTR, bits 5:4 of [`INTERRUPT`]: the interrupt mode (DS Figure 19, p. 17; Figure 20, p. 18).
pub const INTERRUPT_INTR_MASK: u8 = 0b0011_0000;

/// The bit position of the INTR field within [`INTERRUPT`].
pub const INTERRUPT_INTR_SHIFT: u8 = 4;

/// PERSIST, bits 3:0 of [`INTERRUPT`]: how many consecutive integration periods out of the
/// threshold window it takes to assert the interrupt (DS Figure 19, p. 17; Figure 21, p. 18).
pub const INTERRUPT_PERSIST_MASK: u8 = 0b0000_1111;

/// THLLOW, `03h`, read/write, reset `00h`: low byte of the low interrupt threshold (DS
/// Figure 15, p. 12; Figure 22, p. 19).
pub const THLLOW: u8 = 0x03;

/// THLHIGH, `04h`, read/write, reset `00h`: high byte of the low interrupt threshold (DS
/// Figure 15, p. 12; Figure 22, p. 19).
pub const THLHIGH: u8 = 0x04;

/// THHLOW, `05h`, read/write, reset `00h`: low byte of the high interrupt threshold (DS
/// Figure 15, p. 12; Figure 22, p. 19).
pub const THHLOW: u8 = 0x05;

/// THHHIGH, `06h`, read/write, reset `00h`: high byte of the high interrupt threshold (DS
/// Figure 15, p. 12; Figure 22, p. 19).
pub const THHHIGH: u8 = 0x06;

/// ANALOG, `07h`, read/write, reset `00h`: the analog gain (DS Figure 15, p. 12; Figure 23,
/// p. 20).
pub const ANALOG: u8 = 0x07;

/// GAIN, bits 1:0 of [`ANALOG`]: one of the [`Gain`](crate::Gain) values (DS Figure 23, p. 20).
/// Bits 7:2 are "Reserved. Write as 0", so the driver writes the whole register.
pub const ANALOG_GAIN_MASK: u8 = 0b0000_0011;

/// ID, `12h`, read only: part number and silicon revision (DS Figure 15, p. 12; Figure 24,
/// p. 20).
pub const ID: u8 = 0x12;

/// PARTNO, bits 7:4 of [`ID`] (DS Figure 24, p. 20).
pub const ID_PARTNO_MASK: u8 = 0b1111_0000;

/// How far PARTNO sits from bit 0 of [`ID`] (DS Figure 24, p. 20).
pub const ID_PARTNO_SHIFT: u8 = 4;

/// The PARTNO value of the TSL2584TSV: "Part Number Identification: field value 1001b" (DS
/// Figure 24, p. 20). The whole family shares this part number,
/// for every part of its tsl258x family.
pub const ID_PARTNO_TSL2584TSV: u8 = 0b1001;

/// REVNO, bits 3:0 of [`ID`]: "Revision number identification" (DS Figure 24, p. 20). The
/// datasheet gives no value for it.
pub const ID_REVNO_MASK: u8 = 0b0000_1111;

/// DATA0LOW, `14h`, read only, reset `00h`: lower byte of ADC channel 0 (DS Figure 15, p. 12;
/// Figure 25, p. 21).
///
/// Reading a lower byte strobes the channel's upper eight bits into a shadow register, which the
/// next read of the upper byte returns; the upper byte "can only be read following a read to the
/// corresponding lower byte register" (DS p. 21). Read low first, then high.
pub const DATA0LOW: u8 = 0x14;

/// DATA0HIGH, `15h`, read only, reset `00h`: upper byte of ADC channel 0 (DS Figure 15, p. 12;
/// Figure 25, p. 21).
pub const DATA0HIGH: u8 = 0x15;

/// DATA1LOW, `16h`, read only, reset `00h`: lower byte of ADC channel 1 (DS Figure 15, p. 12;
/// Figure 25, p. 21).
pub const DATA1LOW: u8 = 0x16;

/// DATA1HIGH, `17h`, read only, reset `00h`: upper byte of ADC channel 1 (DS Figure 15, p. 12;
/// Figure 25, p. 21).
pub const DATA1HIGH: u8 = 0x17;

/// TIMERLOW, `18h`, read only, reset `00h`: lower byte of the manual integration timer, which
/// counts a manual integration in 10.9 µs steps (DS Figure 15, p. 12; Figure 26, p. 22).
pub const TIMERLOW: u8 = 0x18;

/// TIMERHIGH, `19h`, read only, reset `00h`: upper byte of the manual integration timer (DS
/// Figure 15, p. 12; Figure 26, p. 22).
pub const TIMERHIGH: u8 = 0x19;

/// ID2, `1Eh`: "Supplemental identification" (DS Figure 15, p. 12; Figure 27, p. 22).
///
/// The register map lists it as read/write, and the text warns: "it is strongly advised that
/// this register not be written to. Any value written to this register could adversely affect
/// the performance of the device" (DS p. 22). This crate only reads it.
pub const ID2: u8 = 0x1e;

/// ID2, bit 7 of the register [`ID2`]: "This bit will be set (1) for all TSL2584TSV devices" (DS
/// Figure 27, p. 22). Bits 6:0 are reserved. This bit tells the TSL2584TSV from its siblings.
pub const ID2_TSL2584TSV: u8 = 0b1000_0000;

/// The command byte that selects `register` for single-register access: CMD set, TRANSACTION
/// `00`, the address in bits 4:0 (DS Figure 16, p. 13 and 14).
pub const fn command(register: u8) -> u8 {
    COMMAND_CMD | COMMAND_TRANSACTION_REPEATED_BYTE | register
}

/// The command byte that selects `register` as the first of several to be read in one transfer:
/// CMD set, TRANSACTION `01` (auto-increment), the address in bits 4:0 (DS Figure 16, p. 13 and
/// 14).
pub const fn command_auto_increment(register: u8) -> u8 {
    COMMAND_CMD | COMMAND_TRANSACTION_AUTO_INCREMENT | register
}

/// The one-byte command that runs a special function (`function` one of the `SPECIAL_*`
/// values): CMD set, TRANSACTION `11`, the function in bits 4:0 (DS Figure 16, p. 13 and 14).
pub const fn command_special(function: u8) -> u8 {
    COMMAND_CMD | COMMAND_TRANSACTION_SPECIAL_FUNCTION | function
}

// The two functions above OR the address into the command without masking it. That is only right
// while every register address fits the 5-bit ADDRESS field: a larger one would spill into
// TRANSACTION and turn a register access into a special function. ID2 is the highest address in
// the map (DS Figure 15, p. 12), so checking it covers every register.
const _: () = assert!(ID2 & !COMMAND_ADDRESS_MASK == 0);

// The channel read relies on the four data registers being consecutive in the order low, high,
// low, high (DS Figure 25, p. 21), because it reads them with address auto-increment.
const _: () = assert!(DATA0HIGH == DATA0LOW + 1);
const _: () = assert!(DATA1LOW == DATA0LOW + 2);
const _: () = assert!(DATA1HIGH == DATA0LOW + 3);

// The command bytes the driver sends, spelled out, so that a slip in a mask above shows up at
// compile time: 0x80 selects CONTROL, 0x87 ANALOG, 0xB4 the
// channel data with auto-increment.
const _: () = assert!(command(CONTROL) == 0x80);
const _: () = assert!(command(ANALOG) == 0x87);
const _: () = assert!(command_auto_increment(DATA0LOW) == 0xb4);
const _: () = assert!(command(INTERRUPT) == 0x82);
const _: () = assert!(command_auto_increment(THLLOW) == 0xa3);
const _: () = assert!(command_auto_increment(THHLOW) == 0xa5);
const _: () = assert!(command_special(SPECIAL_INTERRUPT_CLEAR) == 0xe1);

// The threshold write relies on each pair being consecutive, low byte then high byte (DS
// Figure 22, p. 19), because it writes the pair with address auto-increment.
const _: () = assert!(THLHIGH == THLLOW + 1);
const _: () = assert!(THHLOW == THLLOW + 2);
const _: () = assert!(THHHIGH == THLLOW + 3);
