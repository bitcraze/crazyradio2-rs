mod crazyradio2;
mod error;
mod rpc;
mod usb_protocol;

pub use crazyradio2::Crazyradio2;
pub use error::Error;

pub type Result<T> = std::result::Result<T, Error>;
