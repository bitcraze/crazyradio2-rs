use std::{collections::HashMap, time::Duration};

use ciborium::value::Value;
use crazyradio2::Crazyradio2;

fn main() -> anyhow::Result<()> {
    let radio = Crazyradio2::new()?;

    let methods: HashMap<String, u32> = radio
        .rpc
        .call("well-known.methods", Value::Null)?;
    println!("{:#?}", methods);

    radio
        .rpc
        .call("led.set", (false, false, false))?;

    let pressed: bool = radio
        .rpc
        .call("button.get", Value::Null)?;
    dbg!(pressed);

    let available_modes = radio.radio_mode_list()?;
    println!("{:#?}", available_modes);
    radio.radio_mode_set("esb")?;

    let address = [0xe7, 0xe7, 0xe7, 0xad, 0x42];

    for channel in 0..=100 {
        let ack = radio.esb_send_packet(channel, &address, &[0xff])?;

        if ack.acked {
            println!("Found Crazyflie on channel {}, Rssi {}", channel, ack.rssi);
        }
    }

    radio.close();

    std::thread::sleep(Duration::from_secs(1));

    let _radio = Crazyradio2::new()?;

    Ok(())
}
