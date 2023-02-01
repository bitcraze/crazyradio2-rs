use std::collections::HashMap;

use ciborium::{cbor, ser::into_writer, value::Value};
use crazyradio2::Crazyradio2;

fn main() -> anyhow::Result<()> {
    let radio = Crazyradio2::new()?;

    let request = cbor!([0, 0, "well-known.methods", null])?;
    let mut request_bytes = vec![];
    into_writer(&request, &mut request_bytes)?;

    dbg!(&request);

    radio.device.send(&request_bytes)?;
    let data = radio.device.recv()?;

    println!("{:?}", data);

    let response: (u32, u32, Value, HashMap<String, u32>) =
        ciborium::de::from_reader(data.as_slice())?;
    dbg!(&response);

    // dbg!(serde_cbor::from_slice(&data).unwrap());

    Ok(())
}
