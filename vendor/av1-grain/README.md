# av1-grain

[![docs.rs](https://img.shields.io/docsrs/av1-grain?style=for-the-badge)](https://docs.rs/av1-grain)
[![Crates.io](https://img.shields.io/crates/v/av1-grain?style=for-the-badge)](https://crates.io/crates/av1-grain)
[![LICENSE](https://img.shields.io/crates/l/av1-grain?style=for-the-badge)](https://github.com/rust-av/av1-grain/blob/main/LICENSE)

This crate contains helper functions for parsing and generating AV1 film grain data.

This code was originally created for use in rav1e.
It has been moved to this crate so it can be shared with other
AV1 crates that need to deal with film grain.

## Examples

The `generate_photon_noise_params` and `write_grain_table` APIs live behind the
`create` feature. Enable it in your project to produce plain-text photon noise tables
compatible with `svt-av1`, `aomenc`, and similar encoders:

```rust
use av1_grain::{
    generate_photon_noise_params, write_grain_table, NoiseGenArgs, TransferFunction,
};

fn main() -> anyhow::Result<()> {
    // This would apply to the entire video--we can use `u64::MAX` as the end timestamp for simplicity.
    let segment = generate_photon_noise_params(
        0,
        u64::MAX,
        NoiseGenArgs {
            // This setting can range from 100-6400 to adjust the noise strength
            iso_setting: 800,
            width: 1920,
            height: 1080,
            transfer_function: TransferFunction::BT1886,
            chroma_grain: true,
            random_seed: None,
        },
    );

    write_grain_table("example.tbl", &[segment])?;
    Ok(())
}
```

Running this program generates a photon noise table covering the entire video
(`start_time` 0 to `end_time` `u64::MAX`) and stores it in `example.tbl`. The
file extension is arbitrary; `.tbl` is a common choice.
