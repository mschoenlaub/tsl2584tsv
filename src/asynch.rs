//! The same driver over `embedded-hal-async`, for executors such as embassy (feature `async`).
//! Every method sends the transfers of its blocking twin on [`Tsl2584tsv`](crate::Tsl2584tsv),
//! built from the same [`reg`] helpers and decoded by the same private functions; the method docs
//! there hold the datasheet citations, and "Using the driver" in the crate documentation applies
//! unchanged.

use embedded_hal_async::i2c::I2c;

use crate::{
    Channels, DeviceId, Error, Gain, IntegrationTime, InterruptConfig, Status, Thresholds, reg,
};

/// A TSL2584TSV on an `embedded-hal-async` I2C bus. See [`crate::Tsl2584tsv`].
pub struct Tsl2584tsvAsync<I2C> {
    i2c: I2C,
    address: u8,
}

impl<I2C: I2c> Tsl2584tsvAsync<I2C> {
    /// Wraps the bus; talks to nothing until a method is awaited. See
    /// [`Tsl2584tsv::new`](crate::Tsl2584tsv::new).
    pub fn new(i2c: I2C, address: u8) -> Self {
        Self { i2c, address }
    }

    /// Gives the bus back.
    pub fn release(self) -> I2C {
        self.i2c
    }

    /// See [`Tsl2584tsv::device_id`](crate::Tsl2584tsv::device_id).
    pub async fn device_id(&mut self) -> Result<DeviceId, Error<I2C::Error>> {
        let id = self.read_register(reg::ID).await?;
        let id2 = self.read_register(reg::ID2).await?;
        Ok(DeviceId { id, id2 })
    }

    /// See [`Tsl2584tsv::check_id`](crate::Tsl2584tsv::check_id).
    pub async fn check_id(&mut self) -> Result<DeviceId, Error<I2C::Error>> {
        let id = self.device_id().await?;
        if id.is_tsl2584tsv() {
            Ok(id)
        } else {
            Err(Error::UnexpectedId(id))
        }
    }

    /// See [`Tsl2584tsv::status`](crate::Tsl2584tsv::status).
    pub async fn status(&mut self) -> Result<Status, Error<I2C::Error>> {
        Ok(Status::from_bits(self.read_register(reg::CONTROL).await?))
    }

    /// See [`Tsl2584tsv::power_on`](crate::Tsl2584tsv::power_on).
    pub async fn power_on(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, reg::CONTROL_POWER).await
    }

    /// See [`Tsl2584tsv::enable_adc`](crate::Tsl2584tsv::enable_adc).
    pub async fn enable_adc(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, reg::CONTROL_POWER | reg::CONTROL_ADC_EN)
            .await
    }

    /// See [`Tsl2584tsv::power_off`](crate::Tsl2584tsv::power_off).
    pub async fn power_off(&mut self) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::CONTROL, 0x00).await
    }

    /// See [`Tsl2584tsv::gain`](crate::Tsl2584tsv::gain).
    pub async fn gain(&mut self) -> Result<Gain, Error<I2C::Error>> {
        Ok(Gain::from_bits(self.read_register(reg::ANALOG).await?))
    }

    /// See [`Tsl2584tsv::set_gain`](crate::Tsl2584tsv::set_gain).
    pub async fn set_gain(&mut self, gain: Gain) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::ANALOG, gain as u8).await
    }

    /// See [`Tsl2584tsv::integration_time`](crate::Tsl2584tsv::integration_time).
    pub async fn integration_time(&mut self) -> Result<Option<IntegrationTime>, Error<I2C::Error>> {
        Ok(IntegrationTime::from_atime(
            self.read_register(reg::TIMING).await?,
        ))
    }

    /// See [`Tsl2584tsv::set_integration_time`](crate::Tsl2584tsv::set_integration_time).
    pub async fn set_integration_time(
        &mut self,
        time: IntegrationTime,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::TIMING, time.atime()).await
    }

    /// See [`Tsl2584tsv::channels`](crate::Tsl2584tsv::channels).
    pub async fn channels(&mut self) -> Result<Channels, Error<I2C::Error>> {
        let mut data = [0u8; 4];
        self.i2c
            .write_read(
                self.address,
                &[reg::command_auto_increment(reg::DATA0LOW)],
                &mut data,
            )
            .await
            .map_err(Error::Bus)?;
        Ok(Channels::from_data(data))
    }

    /// See [`Tsl2584tsv::interrupt`](crate::Tsl2584tsv::interrupt).
    pub async fn interrupt(&mut self) -> Result<InterruptConfig, Error<I2C::Error>> {
        let value = self.read_register(reg::INTERRUPT).await?;
        InterruptConfig::from_bits(value).ok_or(Error::ReservedInterruptMode(value))
    }

    /// See [`Tsl2584tsv::set_interrupt`](crate::Tsl2584tsv::set_interrupt).
    pub async fn set_interrupt(
        &mut self,
        config: InterruptConfig,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_register(reg::INTERRUPT, config.to_bits()).await
    }

    /// See [`Tsl2584tsv::thresholds`](crate::Tsl2584tsv::thresholds).
    pub async fn thresholds(&mut self) -> Result<Thresholds, Error<I2C::Error>> {
        let mut data = [0u8; 4];
        self.i2c
            .write_read(
                self.address,
                &[reg::command_auto_increment(reg::THLLOW)],
                &mut data,
            )
            .await
            .map_err(Error::Bus)?;
        Ok(Thresholds::from_data(data))
    }

    /// See [`Tsl2584tsv::set_thresholds`](crate::Tsl2584tsv::set_thresholds).
    pub async fn set_thresholds(
        &mut self,
        thresholds: Thresholds,
    ) -> Result<(), Error<I2C::Error>> {
        self.write_word(reg::THLLOW, thresholds.low).await?;
        self.write_word(reg::THHLOW, thresholds.high).await
    }

    /// See [`Tsl2584tsv::clear_interrupt`](crate::Tsl2584tsv::clear_interrupt).
    pub async fn clear_interrupt(&mut self) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(
                self.address,
                &[reg::command_special(reg::SPECIAL_INTERRUPT_CLEAR)],
            )
            .await
            .map_err(Error::Bus)
    }

    /// See the blocking driver's `read_register`.
    async fn read_register(&mut self, register: u8) -> Result<u8, Error<I2C::Error>> {
        let mut buf = [0u8];
        self.i2c
            .write_read(self.address, &[reg::command(register)], &mut buf)
            .await
            .map_err(Error::Bus)?;
        Ok(buf[0])
    }

    /// See the blocking driver's `write_register`.
    async fn write_register(&mut self, register: u8, value: u8) -> Result<(), Error<I2C::Error>> {
        self.i2c
            .write(self.address, &[reg::command(register), value])
            .await
            .map_err(Error::Bus)
    }

    /// See the blocking driver's `write_word`.
    async fn write_word(&mut self, register: u8, value: u16) -> Result<(), Error<I2C::Error>> {
        let [low, high] = value.to_le_bytes();
        self.i2c
            .write(
                self.address,
                &[reg::command_auto_increment(register), low, high],
            )
            .await
            .map_err(Error::Bus)
    }
}

// One test per async write path, mirroring the blocking tests (the read paths are checked on a
// chip, README "Testing"): the mock's async impls are the blocking ones underneath, so these pin
// that the async driver sends the same bytes in the same order.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InterruptMode, Persistence};
    use embassy_futures::block_on;
    use embedded_hal_mock::eh1::i2c::{Mock, Transaction};

    const ADDRESS: u8 = reg::ADDRESS_ADDR_SEL_GND;

    #[test]
    fn power_on_enable_adc_and_power_off_write_the_control_register() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x80, 0x01]),
            Transaction::write(ADDRESS, vec![0x80, 0x03]),
            Transaction::write(ADDRESS, vec![0x80, 0x00]),
        ];
        let mut sensor = Tsl2584tsvAsync::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(sensor.power_on()), Ok(()));
        assert_eq!(block_on(sensor.enable_adc()), Ok(()));
        assert_eq!(block_on(sensor.power_off()), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn set_gain_and_set_integration_time_write_their_registers() {
        // 0x87 ANALOG with 111x; 0x81 TIMING with 74 cycles, 256 - 74 = 182 = 0xb6.
        let expectations = [
            Transaction::write(ADDRESS, vec![0x87, 0x03]),
            Transaction::write(ADDRESS, vec![0x81, 0xb6]),
        ];
        let mut sensor = Tsl2584tsvAsync::new(Mock::new(&expectations), ADDRESS);
        assert_eq!(block_on(sensor.set_gain(Gain::X111)), Ok(()));
        let cycles_74 = IntegrationTime::from_cycles(74).expect("74 is not zero");
        assert_eq!(block_on(sensor.set_integration_time(cycles_74)), Ok(()));
        sensor.release().done();
    }

    #[test]
    fn interrupt_writes_send_the_blocking_drivers_bytes() {
        let expectations = [
            Transaction::write(ADDRESS, vec![0x82, 0x1a]),
            Transaction::write(ADDRESS, vec![0xa3, 0x03, 0x00]),
            Transaction::write(ADDRESS, vec![0xa5, 0xff, 0xff]),
            Transaction::write(ADDRESS, vec![0xe1]),
        ];
        let mut sensor = Tsl2584tsvAsync::new(Mock::new(&expectations), ADDRESS);
        let config = InterruptConfig {
            mode: InterruptMode::Level,
            persistence: Persistence::periods(10).expect("10 is 1 to 15"),
            stop_on_interrupt: false,
        };
        assert_eq!(block_on(sensor.set_interrupt(config)), Ok(()));
        let window = Thresholds {
            low: 3,
            high: 0xffff,
        };
        assert_eq!(block_on(sensor.set_thresholds(window)), Ok(()));
        assert_eq!(block_on(sensor.clear_interrupt()), Ok(()));
        sensor.release().done();
    }
}
