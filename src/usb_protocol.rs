// Crazyradio 2 USB protocol implementation
// The USB protocol handles individuals USB packets and abstracts packet
// size to make it appear as a high MTU-capable link.

use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::{Error, Result};

use rusb::{DeviceHandle, GlobalContext, Interfaces};

use std::thread::spawn;

use flume::{Receiver, Sender};

const USB_MTU: usize = 64;
const PROTOCOL_MTU: usize = 1024;

const PROTOCOL_VERSION: u8 = 0;

pub struct UsbProtocol {
    tx_queue: Sender<Vec<u8>>,
    rx_queue: Receiver<Vec<u8>>,
    rx_thread: JoinHandle<Result<()>>,
    tx_thread: JoinHandle<Result<()>>,
    close: Arc<AtomicBool>,
}

fn find_endpoints(mut interfaces: Interfaces) -> Result<(u8, u8)> {
    // Only look at first interface
    let interface = interfaces.next().ok_or(Error::UsbDescriptorError(
        "Cannot find first configuration".to_string(),
    ))?;

    // Only get first setting
    let descriptor = interface
        .descriptors()
        .find(|descriptor| descriptor.class_code() == 0xff && descriptor.sub_class_code() == 0)
        .ok_or(Error::UsbDescriptorError(
            "Interface descriptor not found".to_string(),
        ))?;

    let in_ep = descriptor
        .endpoint_descriptors()
        .find(|endpoint| endpoint.direction() == rusb::Direction::In)
        .ok_or(Error::UsbDescriptorError(
            "IN endpoint not found".to_string(),
        ))?
        .address();

    let out_ep = descriptor
        .endpoint_descriptors()
        .find(|endpoint| endpoint.direction() == rusb::Direction::Out)
        .ok_or(Error::UsbDescriptorError(
            "OUT endpoint not found".to_string(),
        ))?
        .address();

    Ok((in_ep, out_ep))
}

impl UsbProtocol {
    pub fn new(device: DeviceHandle<GlobalContext>) -> crate::Result<Self> {
        // Verify protocol version
        let mut version_buffer = [0];
        device.read_control(
            0xC1,
            0,
            0,
            0,
            &mut version_buffer,
            Duration::from_millis(100),
        )?;

        if version_buffer[0] != PROTOCOL_VERSION {
            return Err(Error::ProtocolVersionError(
                PROTOCOL_VERSION,
                version_buffer[0],
            ));
        }

        // Extract endpoints addresse from the descriptor
        let (in_endpoint, out_endpoint) =
            find_endpoints(device.device().active_config_descriptor()?.interfaces())?;
        let device = Arc::new(device);

        // Reset protocol
        UsbProtocol::reset(device.as_ref(), in_endpoint, out_endpoint)?;

        // Closing flag
        let close = Arc::new(AtomicBool::new(false));

        // TX thread
        let (tx_queue, tx_rcv) = flume::unbounded::<Vec<u8>>();
        let rx_thread = {
            let device = device.clone();
            let close = close.clone();
            spawn::<_, Result<()>>(move || {
                let mut leftover_packet = None;
                loop {
                    if close.load(std::sync::atomic::Ordering::Relaxed) {
                        return Ok(());
                    }

                    // Use previously unqueued packet if available, wait for a new one otherwise
                    let packet = if let Some(packet) = leftover_packet.take() {
                        packet
                    } else {
                        // Never block forever to be able to look at the close flag
                        match tx_rcv.recv_timeout(Duration::from_millis(100)) {
                            Ok(pk) => pk,
                            Err(flume::RecvTimeoutError::Timeout) => continue,
                            Err(flume::RecvTimeoutError::Disconnected) => return Ok(()),
                        }
                    };

                    let mut buffer = (packet.len() as u16).to_le_bytes().to_vec();
                    buffer.extend_from_slice(&packet);

                    // Try to receive more packets from the queue until none are available or the buffer will go over PROTOCOL_MTU
                    while let Ok(packet) = tx_rcv.try_recv() {
                        if buffer.len() + 2 + packet.len() > PROTOCOL_MTU {
                            leftover_packet = Some(packet);
                            break;
                        }
                        buffer.extend_from_slice(&(packet.len() as u16).to_le_bytes());
                        buffer.extend_from_slice(&packet);
                    }

                    // println!("Sending: {:?}", &buffer);

                    device.write_bulk(out_endpoint, &buffer, Duration::from_millis(100))?;
                }
            })
        };

        // RX thread
        let (rx_snd, rx_queue) = flume::unbounded::<Vec<u8>>();
        let tx_thread = {
            let close = close.clone();
            let mut leftover: Option<Vec<u8>> = None;
            spawn::<_, Result<()>>(move || {
                let mut buffer = [0u8; PROTOCOL_MTU];
                loop {
                    let len = loop {
                        if close.load(std::sync::atomic::Ordering::Relaxed) {
                            return Ok(());
                        }

                        // println!("Receiving ...");
                        match device.read_bulk(in_endpoint, &mut buffer, Duration::from_millis(100))
                        {
                            Ok(len) => break len,
                            Err(rusb::Error::Timeout) => continue,
                            Err(e) => return Err(Error::UsbError(e)),
                        }
                    };

                    // println!("Received: {:?} (len {})", &buffer[..len], len);

                    let to_handle = if let Some(leftover) = leftover.take() {
                        Vec::from_iter(leftover.into_iter().chain((&buffer[..len]).iter().cloned()))
                    } else {
                        buffer[..len].to_vec()
                    };
                    let mut slice = to_handle.as_slice();

                    while !slice.is_empty() {
                        let packet_len =
                            u16::from_le_bytes(slice[..2].try_into().unwrap()) as usize;

                        if slice.len() < (packet_len + 2) {
                            leftover = Some(Vec::from_iter(slice.iter().cloned()));
                            break;
                        }
                        let packet = slice[2..2 + packet_len].to_vec();
                        slice = &slice[2 + packet_len..];

                        rx_snd.send(packet)?;
                    }

                    // println!("Leftover: {:?}", &leftover);
                }
            })
        };

        Ok(UsbProtocol {
            tx_queue,
            rx_queue,
            rx_thread,
            tx_thread,
            close,
        })
    }

    fn reset(
        device: &DeviceHandle<GlobalContext>,
        in_endpoint: u8,
        out_endpoint: u8,
    ) -> Result<()> {
        // Reset out by sending a 0 byte packet
        device.write_bulk(out_endpoint, &[], Duration::from_millis(100))?;

        // Reset in by reading all available packets until we get a < USB_MTU byte packet
        let mut buffer = [0u8; USB_MTU];
        loop {
            let len = match device.read_bulk(in_endpoint, &mut buffer, Duration::from_millis(10)) {
                Ok(len) => Ok(len),
                Err(rusb::Error::Timeout) => Ok(0),
                Err(e) => Err(Error::UsbError(e)),
            }?;

            if len < USB_MTU {
                break;
            }
        }

        Ok(())
    }

    pub fn send(&self, data: &[u8]) -> Result<()> {
        self.tx_queue
            .send(data.to_vec())
            .map_err(Error::DeviceTxError)
    }

    pub fn recv(&self) -> Result<Vec<u8>> {
        self.rx_queue.recv().map_err(Error::DeviceRxError)
    }

    pub(crate) fn close(&self) {
        self.close.store(true, std::sync::atomic::Ordering::Relaxed);

        // This madness is required to keep the close function taking &self
        // but still waiting for the threads to close
        while !self.tx_thread.is_finished() || !self.rx_thread.is_finished() {
            thread::sleep(Duration::from_millis(10));
        }
    }
}
