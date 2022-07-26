<!-- SPDX-FileCopyrightText: The mpeg-audio-header authors -->
<!-- SPDX-License-Identifier: MPL-2.0 -->

# mpeg-audio-header

[![Crates.io](https://img.shields.io/crates/v/mpeg-audio-header.svg)](https://crates.io/crates/mpeg-audio-header)
[![Docs.rs](https://docs.rs/mpeg-audio-header/badge.svg)](https://docs.rs/mpeg-audio-header)
[![Deps.rs](https://deps.rs/repo/github/uklotzde/mpeg-audio-header/status.svg)](https://deps.rs/repo/github/uklotzde/mpeg-audio-header)
[![Security audit](https://github.com/uklotzde/mpeg-audio-header/actions/workflows/security-audit.yaml/badge.svg)](https://github.com/uklotzde/mpeg-audio-header/actions/workflows/security-audit.yaml)
[![Continuous integration](https://github.com/uklotzde/mpeg-audio-header/actions/workflows/continuous-integration.yaml/badge.svg)](https://github.com/uklotzde/mpeg-audio-header/actions/workflows/continuous-integration.yaml)
[![License: MPL 2.0](https://img.shields.io/badge/License-MPL_2.0-brightgreen.svg)](https://opensource.org/licenses/MPL-2.0)

Parse metadata of an MPEG audio stream from VBR (XING/VBRI) and MPEG frame headers.

## Motivation

The specification of the MPEG audio format is very weak. There is no dedicated header that
contains consistent metadata of the encoded audio stream like the number of channels,
the sample rate (Hz), or the average bitrate (bits per second) for estimating the
compression ratio and audio quality.

This library aims to determine audio metadata by applying a best effort heuristic.
The metadata is either contained in a VBR header (XING/VBRI) that precedes the audio
frames or it could be collected and aggregated from all MPEG frame headers to obtain
more precise and reliable information.

## Limitations

The metadata parser has deliberately been designed as fault tolerant and
may provide results even for corrupt or invalid files. The accuracy of such
results is undefined. A more restritive parsing strategy with respective
error reporting might be added in the future. Currently only I/O errors
could stop the parser from continuing.

This crate does not aim to parse ID3 or APE metadata and never will.
Use a dedicated crate like [id3](https://crates.io/crates/id3) or
[ape](https://crates.io/crates/ape) for this purpose.

## Testing

Test files are expected to be available in the [test-data/](./test/data/) directory.
They are not provided as part of this repository and currently need to be downloaded
manually. Please refer to the `.keep` file in each directory which contains the
respective download URL. Automatically downloading the test files on demand would
be awesome.

The test files are referred to by their path. If no dedicated checks for the
resulting header contents are provided then only reading the header of those
files must succeed. Expected failures for corrupt files could also be verified.

Run the tests with `-- --nocapture` for diagnostic output on `stdout`/`stderr`.

## Credits

This crate initially started as a fork of [mp3-duration](https://crates.io/crates/mp3-duration).
Soon it became obvious that a substantial rewrite was necessary to cope with the new
requirements and to properly handle all format variants corrrectly. Yet some code
fragments may still reflect that heritage.

Some ideas have also been borrowed from [symphonia](https://crates.io/crates/symphonia)
and [lofty-rs](https://github.com/Serial-ATA/lofty-rs).

## License

Licensed under the Mozilla Public License 2.0 (MPL-2.0) (see [MPL-2.0.txt](LICENSES/MPL-2.0.txt) or <https://www.mozilla.org/MPL/2.0/>).

Permissions of this copyleft license are conditioned on making available source code of licensed files and modifications of those files under the same license (or in certain cases, one of the GNU licenses). Copyright and license notices must be preserved. Contributors provide an express grant of patent rights. However, a larger work using the licensed work may be distributed under different terms and without source code for files added in the larger work.

### Contribution

Any contribution intentionally submitted for inclusion in the work by you shall be licensed under the Mozilla Public License 2.0 (MPL-2.0).

It is required to add the following header with the corresponding [SPDX short identifier](https://spdx.dev/ids/) to the top of each file:

```rust
// SPDX-License-Identifier: MPL-2.0
```
