//! mpeg-audio-header
//!
//! Parse metadata of an MPEG audio stream from VBR (XING/VBRI) and MPEG frame headers.

#![warn(unsafe_code)]
#![cfg_attr(not(debug_assertions), deny(warnings))]
#![deny(rust_2018_idioms)]
#![deny(rust_2021_compatibility)]
#![deny(missing_debug_implementations)]
#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![deny(clippy::all)]
#![deny(clippy::explicit_deref_methods)]
#![deny(clippy::explicit_into_iter_loop)]
#![deny(clippy::explicit_iter_loop)]
#![deny(clippy::must_use_candidate)]
#![cfg_attr(test, deny(warnings))]
#![cfg_attr(not(test), deny(clippy::panic_in_result_fn))]
#![cfg_attr(not(debug_assertions), deny(clippy::used_underscore_binding))]

use std::{
    fs::File,
    io::{BufReader, Read},
    path::Path,
    time::Duration,
};

mod error;
mod frame;
mod reader;

pub use self::frame::{Layer, Mode, Version};

use self::frame::{FrameHeader, XING_HEADER_MIN_SIZE, XING_VBRI_HEADER_MIN_SIZE};

use self::reader::Reader;

pub use self::{
    error::{Error, PositionalError},
    reader::ReadPosition,
};

/// Result type for [`PositionalError`]
pub type PositionalResult<T> = std::result::Result<T, PositionalError>;

#[derive(Debug, Clone)]
/// Properties of an MPEG audio stream
///
/// A virtual MPEG audio header, built from both the XING header and
/// optionally aggregated from all valid MPEG frame headers.
pub struct Header {
    /// Source of the metadata in this header
    pub source: HeaderSource,

    /// MPEG version
    ///
    /// The common MPEG version in all frames or `None` if either unknown or inconsistent.
    pub version: Option<Version>,

    /// MPEG layer
    ///
    /// The common MPEG layer in all frames or `None` if either unknown or inconsistent.
    pub layer: Option<Layer>,

    /// MPEG mode
    ///
    /// The common MPEG mode in all frames or `None` if either unknown or inconsistent.
    pub mode: Option<Mode>,

    /// Minimum number of channels
    pub min_channel_count: u8,

    /// Maximum number of channels
    pub max_channel_count: u8,

    /// Minimum sample rate in Hz
    pub min_sample_rate_hz: u16,

    /// Maximum sample rate in Hz
    pub max_sample_rate_hz: u16,

    /// Total number of samples per channel
    pub total_sample_count: u64,

    /// Total duration
    pub total_duration: Duration,

    /// Average sample rate in Hz
    pub avg_sample_rate_hz: Option<u16>,

    /// Average bitrate in bits/sec
    pub avg_bitrate_bps: Option<u32>,
}

/// Parse mode
///
/// Controls which sources are considered when parsing metadata.
#[derive(Debug, Clone, Copy)]
pub enum ParseMode {
    /// Parse from first VBR header
    ///
    /// If present return the metadata contained in the first valid
    /// XING/VBRI header and abort reading. Otherwise continue reading
    /// and aggregate the metadata from all MPEG audio frames.
    ///
    /// This method is faster but might result in less accurate results
    /// if the information in the VBR headers does not match the data
    /// in the MPEG audio frames.
    PreferVbrHeaders,

    /// Skip and ignore all VBR headers
    ///
    /// Skip over the XING/VBRI headers and aggregate the metadata from
    /// all MPEG audio frames instead.
    ///
    /// This method is slower but may provide more accurate results depending
    /// on how and when the redundant information in the VBR headers has been
    /// calculated.
    IgnoreVbrHeaders,
}

/// Source of the parsed metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderSource {
    /// XING header
    XingHeader,

    /// VBRI header
    VbriHeader,

    /// MPEG audio frames
    MpegFrameHeaders,
}

const NANOS_PER_SECOND: u32 = 1_000_000_000;

impl Header {
    /// Read from a `source` that implements `Read`
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::{path::Path, fs::File, io::BufReader};
    /// use mpeg_audio_header::{Header, ParseMode};
    ///
    /// let path = Path::new("test/source.mp3");
    /// let file = File::open(path).unwrap();
    /// let mut source = BufReader::new(file);
    /// let header = Header::read_from_source(&mut source, ParseMode::IgnoreVbrHeaders).unwrap();
    /// println!("MPEG audio header: {:?}", header);
    /// ```
    pub fn read_from_source(
        source: &mut impl Read,
        parse_mode: ParseMode,
    ) -> PositionalResult<Self> {
        let mut reader = Reader::new(source);

        let mut version = None;
        let mut version_consistent = true;

        let mut layer = None;
        let mut layer_consistent = true;

        let mut mode = None;
        let mut mode_consistent = true;

        let mut min_channel_count = 0;
        let mut max_channel_count = 0;

        let mut sum_sample_count = 0u64;

        let mut min_sample_rate_hz = 0;
        let mut max_sample_rate_hz = 0;
        let mut accmul_sample_rate_hz = 0u64;

        let mut min_bitrate_bps = 0;
        let mut max_bitrate_bps = 0;
        let mut accmul_bitrate_bps = 0u64;

        loop {
            let next_read_res = match FrameHeader::try_read(&mut reader) {
                Ok(res) => res,
                Err(err) => {
                    if err.is_unexpected_eof() && sum_sample_count > 0 {
                        // Silently ignore all unrecognized data after at least one
                        // non-empty MPEG frame has been parsed.
                        break;
                    }
                    return Err(err);
                }
            };
            match next_read_res {
                Ok(Some(frame_header)) => {
                    // MPEG frame
                    let mut num_bytes_consumed = u32::from(frame::FRAME_HEADER_SIZE);
                    if !reader
                        .try_skip_exact_until_eof(u64::from(frame_header.side_information_size()))?
                    {
                        break;
                    }
                    num_bytes_consumed += u32::from(frame_header.side_information_size());

                    let mut is_audio_frame = true;

                    // XING header frames may only appear at the start of the file before
                    // the first MPEG frame with audio data.
                    debug_assert!(frame_header.check_payload_size(num_bytes_consumed as u16));
                    if sum_sample_count == 0
                        && frame_header.check_payload_size(
                            num_bytes_consumed as u16 + u16::from(XING_HEADER_MIN_SIZE),
                        )
                    {
                        let mut xing_header = [0; XING_HEADER_MIN_SIZE as usize];
                        if !reader.try_read_exact_until_eof(&mut xing_header)? {
                            break;
                        }
                        num_bytes_consumed += u32::from(XING_HEADER_MIN_SIZE);

                        let mut vbr_total_frames: Option<(HeaderSource, u32)> = None;
                        match &xing_header[..4] {
                            // XING header starts with either "Xing" or "Info"
                            // https://www.codeproject.com/Articles/8295/MPEG-Audio-Frame-Header#XINGHeader
                            b"Xing" | b"Info" => {
                                // No audio data in these special frames!
                                is_audio_frame = false;

                                // The XING header must precede all MPEG frames
                                debug_assert!(version.is_none());
                                debug_assert!(layer.is_none());
                                debug_assert!(mode.is_none());

                                if xing_header[7] & 0b0001 != 0 {
                                    // 4 Bytes
                                    let mut total_frames_bytes = [0; 4];
                                    if !reader.try_read_exact_until_eof(&mut total_frames_bytes)? {
                                        break;
                                    }
                                    let total_frames = u32::from_be_bytes(total_frames_bytes);
                                    if total_frames > 0 {
                                        vbr_total_frames =
                                            Some((HeaderSource::XingHeader, total_frames));
                                    }
                                }
                                let mut skip_size = 0u32;
                                if xing_header[7] & 0b0010 != 0 {
                                    // Size
                                    skip_size += 4;
                                }
                                if xing_header[7] & 0b0100 != 0 {
                                    // TOC
                                    skip_size += 100;
                                }
                                if xing_header[7] & 0b1000 != 0 {
                                    // Audio quality
                                    skip_size += 4;
                                }
                                if !reader.try_skip_exact_until_eof(u64::from(skip_size))? {
                                    break;
                                }
                                // Finally finish this frame by pretending that we have consumed all bytes
                                num_bytes_consumed = frame_header
                                    .frame_size
                                    .map(Into::into)
                                    .unwrap_or(num_bytes_consumed);
                            }
                            // https://www.codeproject.com/Articles/8295/MPEG-Audio-Frame-Header#VBRIHeader
                            b"VBRI"
                                if frame_header.check_payload_size(
                                    num_bytes_consumed as u16
                                        + u16::from(XING_VBRI_HEADER_MIN_SIZE),
                                ) =>
                            {
                                // No audio data in these special frames!
                                is_audio_frame = false;

                                // We only read total_frames and skip the rest. The words containing version (2 bytes)
                                // and delay (2 bytes) have already been read into the XING header:
                                // | 4 ("VBRI") + 2 (version) + 2 (delay) + 2 (quality) + 4 (size/bytes) + 4 (total_frames) + ...
                                // |<-         XING Header              ->|<-                 XING/VBRI Header...
                                let mut xing_vbri_header = [0; XING_VBRI_HEADER_MIN_SIZE as usize];
                                if !reader.try_read_exact_until_eof(&mut xing_vbri_header)? {
                                    break;
                                }

                                let total_frames = u32::from_be_bytes(
                                    xing_vbri_header[6..10].try_into().expect("4 bytes"),
                                );
                                if total_frames > 0 {
                                    vbr_total_frames =
                                        Some((HeaderSource::VbriHeader, total_frames));
                                }

                                let toc_entries_count = u16::from_be_bytes(
                                    xing_vbri_header[12..14].try_into().expect("2 bytes"),
                                );

                                let toc_entry_size = u16::from_be_bytes(
                                    xing_vbri_header[16..18].try_into().expect("2 bytes"),
                                );

                                // Skip all trailing TOC entries
                                let toc_size =
                                    u32::from(toc_entries_count) * u32::from(toc_entry_size);
                                if !reader.try_skip_exact_until_eof(u64::from(toc_size))? {
                                    break;
                                }

                                // Finally finish this frame by pretending that we have consumed all bytes
                                num_bytes_consumed = frame_header
                                    .frame_size
                                    .map(Into::into)
                                    .unwrap_or(num_bytes_consumed);
                            }
                            _ => {
                                // Ordinary audio frame
                                debug_assert!(is_audio_frame);
                            }
                        }
                        if let Some((source, total_frames)) = vbr_total_frames {
                            let total_sample_count =
                                u64::from(total_frames) * u64::from(frame_header.sample_count);
                            let seconds =
                                total_sample_count / u64::from(frame_header.sample_rate_hz);
                            let nanoseconds = (total_sample_count * u64::from(NANOS_PER_SECOND))
                                / u64::from(frame_header.sample_rate_hz)
                                - u64::from(NANOS_PER_SECOND) * seconds;
                            debug_assert!(nanoseconds < NANOS_PER_SECOND.into());
                            let total_duration = Duration::new(seconds, nanoseconds as u32);
                            match parse_mode {
                                ParseMode::PreferVbrHeaders => {
                                    return Ok(Self {
                                        source,
                                        version: Some(frame_header.version),
                                        layer: Some(frame_header.layer),
                                        mode: Some(frame_header.mode),
                                        min_channel_count: frame_header.channel_count(),
                                        max_channel_count: frame_header.channel_count(),
                                        min_sample_rate_hz: frame_header.sample_rate_hz,
                                        max_sample_rate_hz: frame_header.sample_rate_hz,
                                        total_sample_count,
                                        total_duration,
                                        avg_sample_rate_hz: Some(frame_header.sample_rate_hz),
                                        avg_bitrate_bps: frame_header.bitrate_bps,
                                    });
                                }
                                ParseMode::IgnoreVbrHeaders => {
                                    // Just skip the VBR headers
                                }
                            }
                        }
                    }
                    if let Some(frame_size) = frame_header.frame_size {
                        debug_assert!(u32::from(frame_size) >= num_bytes_consumed);
                        if !reader.try_skip_exact_until_eof(u64::from(
                            u32::from(frame_size) - num_bytes_consumed,
                        ))? {
                            break;
                        }
                    }

                    if is_audio_frame {
                        if version_consistent {
                            if let Some(some_version) = version {
                                version_consistent = some_version == frame_header.version;
                                if !version_consistent {
                                    version = None;
                                }
                            } else {
                                version = Some(frame_header.version);
                            }
                        }

                        if !layer_consistent {
                            if let Some(some_layer) = layer {
                                layer_consistent = some_layer == frame_header.layer;
                                if !layer_consistent {
                                    layer = None;
                                }
                            } else {
                                layer = Some(frame_header.layer);
                            }
                        }

                        if mode_consistent {
                            if let Some(some_mode) = mode {
                                mode_consistent = some_mode == frame_header.mode;
                                if !mode_consistent {
                                    mode = None;
                                }
                            } else {
                                mode = Some(frame_header.mode);
                            }
                        }

                        let frame_samples = u64::from(frame_header.sample_count);
                        debug_assert!(frame_samples > 0);
                        sum_sample_count += frame_samples;

                        let channel_count = frame_header.channel_count();
                        debug_assert!(channel_count > 0);
                        if min_channel_count == 0 {
                            min_channel_count = channel_count;
                        } else {
                            min_channel_count = min_channel_count.min(channel_count);
                        }
                        if max_channel_count == 0 {
                            max_channel_count = channel_count;
                        } else {
                            max_channel_count = max_channel_count.max(channel_count);
                        }

                        // Free bitrate = 0 bps
                        if let Some(bitrate_bps) = frame_header.bitrate_bps {
                            if min_bitrate_bps == 0 {
                                min_bitrate_bps = bitrate_bps;
                            } else {
                                min_bitrate_bps = min_bitrate_bps.min(bitrate_bps);
                            }
                            if max_bitrate_bps == 0 {
                                max_bitrate_bps = bitrate_bps;
                            } else {
                                max_bitrate_bps = max_bitrate_bps.max(bitrate_bps);
                            }
                            accmul_bitrate_bps += u64::from(bitrate_bps) * frame_samples;
                        }

                        debug_assert!(frame_header.sample_rate_hz > 0);
                        if min_sample_rate_hz == 0 {
                            min_sample_rate_hz = frame_header.sample_rate_hz;
                        } else {
                            min_sample_rate_hz =
                                min_sample_rate_hz.min(frame_header.sample_rate_hz);
                        }
                        if max_sample_rate_hz == 0 {
                            max_sample_rate_hz = frame_header.sample_rate_hz;
                        } else {
                            max_sample_rate_hz =
                                max_sample_rate_hz.max(frame_header.sample_rate_hz);
                        }
                        accmul_sample_rate_hz +=
                            u64::from(frame_header.sample_rate_hz) * frame_samples;

                        let frame_duration_nanos: u64 = (frame_samples
                            * u64::from(NANOS_PER_SECOND))
                            / u64::from(frame_header.sample_rate_hz);
                        debug_assert!(frame_duration_nanos < NANOS_PER_SECOND.into());
                        reader.add_duration(Duration::new(0, frame_duration_nanos as u32));
                    }
                }
                Ok(None) => break,
                Err((frame_header_bytes, header_err)) => {
                    if frame::skip_metadata(&mut reader, frame_header_bytes)? {
                        if sum_sample_count > 0 {
                            // No more MPEG frames after a trailing metadata frame expected
                            break;
                        }
                    } else {
                        return Err(header_err);
                    }
                }
            }
        }

        let total_sample_count = sum_sample_count;
        let total_duration = reader.position().duration;

        let avg_sample_rate_hz = if total_sample_count > 0 {
            let avg_sample_rate_hz = accmul_sample_rate_hz / total_sample_count;
            debug_assert!(avg_sample_rate_hz <= u16::MAX.into());
            Some(avg_sample_rate_hz as u16)
        } else {
            None
        };

        let avg_bitrate_bps = if total_sample_count > 0 {
            let avg_bitrate_bps = accmul_bitrate_bps / total_sample_count;
            debug_assert!(avg_bitrate_bps <= u32::MAX.into());
            Some(avg_bitrate_bps as u32)
        } else {
            None
        };

        Ok(Self {
            source: HeaderSource::MpegFrameHeaders,
            version,
            layer,
            mode,
            min_channel_count,
            max_channel_count,
            min_sample_rate_hz,
            max_sample_rate_hz,
            total_sample_count,
            total_duration,
            avg_sample_rate_hz,
            avg_bitrate_bps,
        })
    }

    /// Read from a file
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::{path::Path, fs::File};
    /// use mpeg_audio_header::{Header, ParseMode};
    ///
    /// let path = Path::new("test/source.mp3");
    /// let file = File::open(path).unwrap();
    /// let header = Header::read_from_file(&file, ParseMode::PreferVbrHeaders).unwrap();
    /// println!("MPEG audio header: {:?}", header);
    /// ```
    pub fn read_from_file(file: &File, parse_mode: ParseMode) -> PositionalResult<Self> {
        let mut source = BufReader::new(file);
        Self::read_from_source(&mut source, parse_mode)
    }

    /// Read from a file path
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use std::path::Path;
    /// use mpeg_audio_header::{Header, ParseMode};
    ///
    /// let path = Path::new("test/source.mp3");
    /// let header = Header::read_from_path(&path, ParseMode::PreferVbrHeaders).unwrap();
    /// println!("MPEG audio header: {:?}", header);
    /// ```
    pub fn read_from_path(path: impl AsRef<Path>, parse_mode: ParseMode) -> PositionalResult<Self> {
        File::open(path)
            .map_err(|e| PositionalError {
                source: e.into(),
                position: ReadPosition::new(),
            })
            .and_then(|file| Self::read_from_file(&file, parse_mode))
    }
}

#[cfg(test)]
mod tests;
