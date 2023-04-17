use crazyradio2::Crazyradio2;
use std::{collections::HashMap, time::Instant};
use rayon::prelude::*;

fn main() -> anyhow::Result<()> {
    let radio = Crazyradio2::new()?;

    let start = Instant::now();

    (0..10000).into_par_iter().for_each(|_| {
        println!(".");
        let _version: HashMap<String, Vec<i64>> =
            radio.rpc.call::<Option<()>, _>("version", None).unwrap();
    });

    let elapsed = start.elapsed();
    let elapsed = elapsed.as_secs_f64();
    let rate = 10000.0 / elapsed;

    println!(
        "{} calls in {} seconds, {} calls per second",
        10000, elapsed, rate
    );

    Ok(())
}
