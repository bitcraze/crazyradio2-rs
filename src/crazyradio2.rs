use ciborium::value::Value;
use rusb::{Device, GlobalContext};

use crate::{error::Error, rpc::Rpc, usb_protocol::UsbProtocol};

use crate::rpc::RpcError;

pub struct EsbAck {
    pub acked: bool,
    pub data: Option<Vec<u8>>,
    pub rssi: u8,
}
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

    pub fn radio_mode_list(&self) -> Result<Vec<String>, RpcError> {
        self.rpc
            .call("radioMode.list", crate::rpc::NULL)
    }

    pub fn radio_mode_set(&self, mode: &str) -> Result<(), RpcError> {
        self.rpc
            .call("radioMode.set", mode)?;
        Ok(())
    }

    pub fn esb_send_packet(
        &self,
        channel: u8,
        address: &[u8; 5],
        data: &[u8],
    ) -> Result<EsbAck, RpcError> {
        let (acked, data, rssi): (bool, Option<Value>, i8) =
            self.rpc.call(
                "esb.sendPacket",
                (
                    channel,
                    Value::Bytes(address.to_vec()),
                    Value::Bytes(data.to_vec()),
                ),
            )?;
        Ok(EsbAck {
            acked,
            data: data.map(|v| v.as_bytes().unwrap().to_owned()),
            rssi: rssi as u8,
        })
    }

    pub fn close(&self) {
        self.rpc.close();
    }
}

impl Drop for Crazyradio2 {
    fn drop(&mut self) {
        self.close();
    }
}
