
# FBD Sequencer

[![Crates.io](https://img.shields.io/crates/v/fbd_sequencer.svg)](https://crates.io/crates/fbd_sequencer)
[![Documentation](https://docs.rs/fbd_sequencer/badge.svg)](https://docs.rs/fbd_sequencer)
[![Build Status](https://github.com/ain1084/rust_fbd_sequencer/workflows/Build/badge.svg)](https://github.com/ain1084/rust_fbd_sequencer/actions?query=workflow%3ABuild)
![Crates.io License](https://img.shields.io/crates/l/fbd_sequencer)

## Overview

This crate implements a sequencer for playing music using PSG or AY-3-8910 sound sources. No actual PSG hardware is required as the PCM is generated and played through software. Currently, it only supports playing .fbd sequence files.

## Installation Instructions

### Using cargo install

```
cargo install fbd_sequencer
```

### Download Pre-built Binaries

Pre-built binaries are available on the [release page of the repository](https://github.com/ain1084/rust_fbd_sequencer/releases).

*!!! Due to the nature of containing only executable files, they may be falsely detected as a virus, leading to complex operations for download or execution. If possible, it is recommended to use cargo install.*

#### Contents of assets
| Filename                                 | Description                          |
|------------------------------------------|--------------------------------------|
| fbdplay-x86_64-unknown-linux-gnu.zip     | fbdplay binary for Linux x64         |
| fbdplay-x86_64-pc-windows-msvc.zip       | fbdplay binary for Windows x64       |
| fpdplay-aarch64-pc-windows-msvc.zip      | fbdplay binary for Windows arm64     |
| fddplay-aarch64-apple-darwin.zip         | fbdplay binary for macOS (with Apple silicon) |
| fbd_files.zip                            | Archive of .fbd files (zip)          |

## How to Use fbdplay

fbdplay is a CLI tool for playing .fbd files. It can output to audio devices and generate .wav files.

The fbd_files can be found in the [fbd_files directory of the repository](https://github.com/ain1084/rust_fbd_sequencer). Alternatively, you can download the fbd_files.zip from the [release page](https://github.com/ain1084/rust_fbd_sequencer/releases).

```
FBD Music player

Usage: fbdplay [OPTIONS] <INPUT> [OUTPUT]

Arguments:
  <INPUT>   Sets the input .fbd file
  [OUTPUT]  Sets the generated .wav file

Options:
  -p, --psg-crate <PSG_CRATE>      Sets the crate for waveform generation [default: psg] [possible values: psg, psg-lite]
  -c, --clock-rate <CLOCK_RATE>    Sets the clock rate (MHz) (e.g., 2.0, 1.7897725...) [default: 2]
  -s, --sample-rate <SAMPLE_RATE>  Sets the sample rate (Hz) [default: 44100]
  -h, --help                       Print help
  -V, --version                    Print version
```

### Example: Playing a file

```
fbdplay fbd_files/YS200.fbd
```

### Example: Generating a .wav file

```
fbdplay fbd_files/YS200.fbd YS200.wav
```

### Example: Specifying a sample rate of 48KHz

```
fbdplay fbd_files/YS200.fbd -s 48000
```

## About .fbd Files

Sequence files are binary files with the .fbd extension. They are designed for PSG sound sources and have three independent channels. The term "FBD" does not have any particular meaning.

Several .fbd files mimicking game music using PSG sound chips are available in the repository. All of them are designed to resemble game music that uses the PSG sound chip on a PC (MSX). (Copyright of the music: © Nihon Falcom Corporation).

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

## About PSG Waveform Generation

To generate PSG waveforms, two crates can be used interchangeably:
* [psg](https://crates.io/crates/psg) (created by Emil Loer)
* [psg-lite](https://crates.io/crates/psg_lite) (created by me)

The crate to be used can be changed with the `-p` option in fbdplay. The psg crate is the default as it generates higher quality waveforms compared to the psg-lite crate. psg-lite is lightweight as the name suggests but sacrifices sound quality (though with a sample rate around 44.1KHz, the difference in sound quality is not extreme).

* The psg crate generates waveforms with clock rate precision internally and then downsamples to the specified sample rate.
* The psg-lite crate thins out the generated waveforms using the clock rate and sample rate. It does not perform downsampling.

When using it on a PC, there are no disadvantages to using the psg crate, but I made it switchable since I implemented it. (The reason for implementing psg-lite was that I was unaware of the existence of the psg crate and for studying Rust).

## License

Licensed under either of
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
