use rusb::{Device, GlobalContext};

use crate::{error::Error, rpc::Rpc, usb_protocol::UsbProtocol};

pub struct Crazyradio2 {
    pub rpc: Rpc,
}

impl Crazyradio2 {
    fn nth_radio_device(n: usize) -> Option<Device<GlobalContext>> {
        let devices = rusb::devices().ok()?;
        devices
            .iter()
            .filter(|device| {
                if let Ok(desc) = device.device_descriptor() {
                    desc.vendor_id() == 0x35f0 && desc.product_id() == 0xad20
                } else {
                    false
                }
            })
            .nth(n)
    }

    pub fn new() -> Result<Crazyradio2, Error> {
        let device = Crazyradio2::nth_radio_device(0).ok_or(Error::CrazyradioNotFound)?;

        let device = device.open().map_err(Error::CannotOpenCrazyradio)?;

        let device = UsbProtocol::new(device)?;

        let rpc = Rpc::new(device);

        Ok(Crazyradio2 { rpc })
    }
}
