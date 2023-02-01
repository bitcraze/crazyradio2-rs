use ciborium::value::Value;
use crazyradio2::Crazyradio2;

fn main() -> anyhow::Result<()> {
    let radio = Crazyradio2::new()?;

    // let methods: HashMap<String, u32> = radio.rpc.call("well-known.methods", Value::Null)?;

    let available_modes: Vec<String> = radio.rpc.call("radioMode.list", Value::Null)?;
    println!("{:#?}", available_modes);
    radio.rpc.call("radioMode.set", "esb")?;

    let address = vec![0xe7, 0xe7, 0xe7, 0xad, 0x42];

    for channel in 0..=100 {
        let (acked, _, rssi): (bool, Option<Value>, i8) = radio.rpc.call(
            "esb.sendPacket",
            (
                channel,
                Value::Bytes(address.clone()),
                Value::Bytes(vec![0xff]),
            ),
        )?;

        if acked {
            println!("Found Crazyflie on channel {}, Rssi {}", channel, rssi);
        }
    }

    Ok(())
}
