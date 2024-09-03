# FBD Sequencer Library

[![Crates.io](https://img.shields.io/crates/v/fbd_sequencer.svg)](https://crates.io/crates/fbd_sequencer)
[![Documentation](https://docs.rs/fbd_sequencer/badge.svg)](https://docs.rs/fbd_sequencer)
[![Build Status](https://github.com/ain1084/rust_fbd_sequencer/workflows/Build/badge.svg)](https://github.com/ain1084/rust_fbd_sequencer/actions?query=workflow%3ABuild)
![Crates.io License](https://img.shields.io/crates/l/fbd_sequencer)

## Overview

This crate implements a sequencer (.fbd) for playing music using PSG or AY-3-8910 sound sources. This library itself does not generate PSG waveforms. The generation of PSG waveforms is delegated to external implementations that implement the `PsgTrait`.

The Web Application using the crate is [fbdplay_wasm](https://github.com/ain1084/fbdplay_wasm), and the CUI Application is [fbd_sequencer_cli](https://crates.io/crates/fbd_sequencer_cli).


## About .fbd Files

Sequence files are binary files with the .fbd extension. They are designed for PSG sound sources and have three independent channels. The term "FBD" does not have any particular meaning.

Several .fbd files mimicking game music using PSG sound chips are available in the repository. All of them are designed to resemble game music that uses the PSG sound chip on a PC (MSX).

Copyright of the music: © Nihon Falcom Corporation.

Currently, there are no tools available to create .fbd files from scratch. We are considering developing tools to convert from formats like MML.

### Features

- Software envelope (AL, AR, DR, SR, SL, RR)
- Software LFO (vibrato)
- Noise period setting
- Output mode setting (tone, noise, tone & noise)
- Tone period offset (also known as detune)
- Nested loops
- Tie and slur
- Fine control in 1/60 second units

## PsgTrait

This library generates PSG waveforms through the following trait.

```
#[derive(PartialEq)]
pub enum OutputMode {
    None,
    Tone,
    Noise,
    ToneNoise,
}

pub trait PsgTrait {
    fn sample_rate(&self) -> u32;
    fn clock_rate(&self) -> u32;
    fn set_tone_period(&mut self, channel: usize, period: u16);
    fn set_volume(&mut self, channel: usize, volume: u8);
    fn set_output_mode(&mut self, channel: usize, mode: OutputMode);
    fn set_noise_period(&mut self, period: u8);
    fn next_sample_i16(&mut self) -> i16;
    #[cfg(feature = "float")]
    fn next_sample_f32(&mut self) -> f32;
}
```

## License

Licensed under either of
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
