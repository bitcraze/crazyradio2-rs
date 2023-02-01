use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("USB error: {0}")]
    UsbError(#[from] rusb::Error),

    #[error("Crazyradio not found")]
    CrazyradioNotFound,

    #[error("Usb descriptor error: {0}")]
    UsbDescriptorError(String),

    #[error("Cannot Open Crazyradio")]
    CannotOpenCrazyradio(#[source] rusb::Error),

    #[error("Protocol version error. Expected {0}, got {1}")]
    ProtocolVersionError(u8, u8),

    #[error("Device Rx error")]
    DeviceRxError(#[from] flume::RecvError),

    #[error("Device Tx error")]
    DeviceTxError(#[from] flume::SendError<Vec<u8>>),
}
