use std::{io::Read, time::Duration};

use crate::{reader::Reader, PositionalError, PositionalResult};

pub const FRAME_HEADER_SIZE: u8 = 4;
pub const XING_HEADER_MIN_SIZE: u8 = 8;
pub const XING_VBRI_HEADER_MIN_SIZE: u8 = 22; // 4 + 8 + 22 = 30 (= start of TOC entries)

// Tag frame/header sizes (including FRAME_HEADER_SIZE)
const ID3V1_FRAME_SIZE: u8 = 128;
const ID3V2_HEADER_SIZE: u8 = 10;
const ID3V2_FOOTER_SIZE: u8 = 10;
const APEV2_HEADER_SIZE: u8 = 32;

const HEADER_WORD_SYNC_MASK: u32 = 0xFFE0_0000;

pub fn is_header_word_synced(header_word: u32) -> bool {
    (header_word & HEADER_WORD_SYNC_MASK) == HEADER_WORD_SYNC_MASK
}

pub fn maybe_valid_header_word(header_word: u32) -> bool {
    if version_from_header_word(header_word).is_none()
        || layer_from_header_word(header_word).is_none()
        || !is_valid_bitrate_bits(bitrate_bits_from_header_word(header_word))
        || !is_valid_sample_rate_bits(sample_rate_bits_from_header_word(header_word))
    {
        return false;
    }
    // Emphasis
    if header_word & 0b11 == 0b10 {
        return false;
    }
    true
}

/// MPEG Version
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    /// MPEG-1
    Mpeg1 = 0,

    /// MPEG-2
    Mpeg2 = 1,

    /// MPEG 2.5
    Mpeg25 = 2,
}

const fn version_index(version: Version) -> usize {
    version as usize
}

fn version_from_header_word(header_word: u32) -> Option<Version> {
    match (header_word >> 19) & 0b11 {
        0b00 => Some(Version::Mpeg25),
        0b01 => None,
        0b10 => Some(Version::Mpeg2),
        0b11 => Some(Version::Mpeg1),
        _ => unreachable!("exhaustive match on version bits not recognized by compiler"),
    }
}

/// MPEG Audio Layer
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    /// Layer I
    Layer1 = 0,

    /// Layer II
    Layer2 = 1,

    /// Layer III
    Layer3 = 2,
}

const fn layer_index(layer: Layer) -> usize {
    layer as usize
}

fn layer_from_header_word(header_word: u32) -> Option<Layer> {
    match (header_word >> 17) & 0b11 {
        0b00 => None,
        0b01 => Some(Layer::Layer3),
        0b10 => Some(Layer::Layer2),
        0b11 => Some(Layer::Layer1),
        _ => unreachable!("exhaustive match on layer bits not recognized by compiler"),
    }
}

/// Channel Mode
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Stereo
    Stereo = 0,

    /// Joint Stereo
    JointStereo = 1,

    /// Dual Channel
    DualChannel = 2,

    /// Mono
    Mono = 3,
}

const fn mode_index(mode: Mode) -> usize {
    mode as usize
}

fn mode_from_header_word(header_word: u32) -> Mode {
    match (header_word >> 6) & 0b11 {
        0b00 => Mode::Stereo,
        0b01 => Mode::JointStereo,
        0b10 => Mode::DualChannel,
        0b11 => Mode::Mono,
        _ => unreachable!("exhaustive match on mode bits not recognized by compiler"),
    }
}

static BIT_RATES_KBPS: [[[u32; 15]; 3]; 3] = [
    [
        [
            // Mpeg1 Layer1
            /*free*/ 0, 32, 64, 96, 128, 160, 192, 224, 256, 288,
            320, 352, 384, 416, 448,
        ],
        [
            // Mpeg1 Layer2
            /*free*/ 0, 32, 48, 56, 64, 80, 96, 112, 128, 160,
            192, 224, 256, 320, 384,
        ],
        [
            // Mpeg1 Layer3
            /*free*/ 0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160,
            192, 224, 256, 320,
        ],
    ],
    [
        [
            // Mpeg2 Layer1
            /*free*/ 0, 32, 48, 56, 64, 80, 96, 112, 128, 144,
            160, 176, 192, 224, 256,
        ],
        [
            // Mpeg2 Layer2
            /*free*/ 0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96,
            112, 128, 144, 160,
        ],
        [
            // Mpeg2 Layer3
            /*free*/ 0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96,
            112, 128, 144, 160,
        ],
    ],
    [
        [
            // Mpeg25 Layer1
            /*free*/ 0, 32, 48, 56, 64, 80, 96, 112, 128, 144,
            160, 176, 192, 224, 256,
        ],
        [
            // Mpeg25 Layer2
            /*free*/ 0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96,
            112, 128, 144, 160,
        ],
        [
            // Mpeg25 Layer3
            /*free*/ 0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96,
            112, 128, 144, 160,
        ],
    ],
];

const BITRATE_BITS_MASK: u8 = 0b1111;

fn bitrate_bits_from_header_word(header_word: u32) -> u8 {
    ((header_word >> 12) & u32::from(BITRATE_BITS_MASK)) as u8
}

fn is_valid_bitrate_bits(bitrate_bits: u8) -> bool {
    bitrate_bits & BITRATE_BITS_MASK < BITRATE_BITS_MASK
}

fn bitrate_bps_from_bits(version: Version, layer: Layer, bitrate_bits: u8) -> u32 {
    debug_assert!(is_valid_bitrate_bits(bitrate_bits));
    1000 * BIT_RATES_KBPS[version_index(version)][layer_index(layer)][bitrate_bits as usize]
}

const SAMPLE_RATES_HZ: [[u16; 3]; 3] = [
    [44100, 48000, 32000], // Mpeg1
    [22050, 24000, 16000], // Mpeg2
    [11025, 12000, 8000],  // Mpeg25
];

const SAMPLE_RATE_BITS_MASK: u8 = 0b11;

fn sample_rate_bits_from_header_word(header_word: u32) -> u8 {
    ((header_word >> 10) & u32::from(SAMPLE_RATE_BITS_MASK)) as u8
}

fn is_valid_sample_rate_bits(sample_rate_bits: u8) -> bool {
    sample_rate_bits & SAMPLE_RATE_BITS_MASK < SAMPLE_RATE_BITS_MASK
}

fn sample_rate_hz_from_bits(version: Version, sample_rate_bits: u8) -> u16 {
    debug_assert!(is_valid_sample_rate_bits(sample_rate_bits));
    SAMPLE_RATES_HZ[version_index(version)][sample_rate_bits as usize]
}

const SAMPLE_COUNT: [[u16; 3]; 3] = [
    [384, 1152, 1152], // Mpeg1
    [384, 1152, 576],  // Mpeg2
    [384, 1152, 576],  // Mpeg25
];

const fn sample_count(version: Version, layer: Layer) -> u16 {
    SAMPLE_COUNT[version_index(version)][layer_index(layer)]
}

const SIDE_INFORMATION_SIZES: [[u16; 4]; 3] = [
    [32, 32, 32, 17], // Mpeg1
    [17, 17, 17, 9],  // Mpeg2
    [17, 17, 17, 9],  // Mpeg25
];

const fn side_information_size(version: Version, mode: Mode) -> u16 {
    SIDE_INFORMATION_SIZES[version_index(version)][mode_index(mode)]
}

#[derive(Debug, Clone)]
pub struct FrameHeader {
    pub version: Version,
    pub layer: Layer,
    pub mode: Mode,
    pub sample_count: u16,
    pub sample_rate_hz: u16,
    pub bitrate_bps: Option<u32>,
    pub frame_size: Option<u16>,
}

impl FrameHeader {
    pub fn check_payload_size(&self, payload_size: u16) -> bool {
        if let Some(frame_size) = self.frame_size {
            payload_size <= frame_size
        } else {
            // If the frame size is unknown we assume that the
            // frame is big enough to carry the payload.
            true
        }
    }
}

pub fn try_read_next_header_word<R: Read>(
    reader: &mut Reader<'_, R>,
) -> PositionalResult<Option<u32>> {
    let mut next_byte_buf = [0u8; 1];
    let mut initial_byte_offset = reader.position().byte_offset;
    let mut frame_header_word = 0u32;
    loop {
        while !is_header_word_synced(frame_header_word) {
            if reader.position().byte_offset - initial_byte_offset >= u64::from(FRAME_HEADER_SIZE)
                && skip_metadata(reader, frame_header_word.to_be_bytes())?
            {
                if reader.position().duration == Duration::ZERO {
                    // Restart the loop after skipping leading metadata frames before the MPEG frames
                    initial_byte_offset = reader.position().byte_offset;
                    frame_header_word = 0u32;
                    continue;
                } else {
                    // Ignore all additional data after the first trailing metadata frame
                    return Ok(None);
                }
            }
            if !reader.try_read_exact_until_eof(&mut next_byte_buf)? {
                return Ok(None);
            }
            frame_header_word = (frame_header_word << 8) | u32::from(next_byte_buf[0]);
        }

        if maybe_valid_header_word(frame_header_word) {
            break;
        }

        // Start next round
        if !reader.try_read_exact_until_eof(&mut next_byte_buf)? {
            return Ok(None);
        }
    }

    debug_assert!(is_header_word_synced(frame_header_word));
    debug_assert!(maybe_valid_header_word(frame_header_word));
    Ok(Some(frame_header_word))
}

pub fn skip_metadata<R: Read>(
    reader: &mut Reader<'_, R>,
    frame_header_bytes: [u8; FRAME_HEADER_SIZE as usize],
) -> PositionalResult<bool> {
    match &frame_header_bytes[..3] {
        b"ID3" => {
            // ID3v2 frame
            let mut id3v2 = [0; (ID3V2_HEADER_SIZE - FRAME_HEADER_SIZE) as usize];
            if !reader.try_read_exact_until_eof(&mut id3v2)? {
                // EOF
                return Ok(true);
            }
            let flags = id3v2[1];
            let footer_size = if 0 != (flags & 0b0001_0000) {
                u32::from(ID3V2_FOOTER_SIZE)
            } else {
                0
            };
            // 32/28-bit synchronization safe integer
            let tag_size = u32::from(id3v2[5])
                | (u32::from(id3v2[4]) << 7)
                | (u32::from(id3v2[3]) << 14)
                | (u32::from(id3v2[2]) << 21);
            reader.try_skip_exact_until_eof((tag_size + footer_size).into())?;
            Ok(true)
        }
        b"TAG" => {
            // ID3v1 frame
            reader.try_skip_exact_until_eof((ID3V1_FRAME_SIZE - FRAME_HEADER_SIZE).into())?;
            Ok(true)
        }
        b"APE" if frame_header_bytes[3] == b'T' => {
            // APEv2 frame
            let mut ape_header = [0; (APEV2_HEADER_SIZE - FRAME_HEADER_SIZE) as usize];
            if !reader.try_read_exact_until_eof(&mut ape_header)? {
                // EOF
                return Ok(true);
            }
            if &ape_header[..4] == b"AGEX" {
                let tag_size = u32::from_le_bytes(ape_header[8..12].try_into().expect("4 bytes"));
                reader.try_skip_exact_until_eof(tag_size.into())?;
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub type UnrecognizedFrameHeaderError = ([u8; FRAME_HEADER_SIZE as usize], PositionalError);

pub type TryReadFrameHeaderOutcome =
    std::result::Result<Option<FrameHeader>, UnrecognizedFrameHeaderError>;

impl FrameHeader {
    pub const fn channel_count(&self) -> u8 {
        match self.mode {
            Mode::Stereo | Mode::JointStereo | Mode::DualChannel => 2,
            Mode::Mono => 1,
        }
    }

    pub fn side_information_size(&self) -> u16 {
        side_information_size(self.version, self.mode)
    }

    #[allow(clippy::panic_in_result_fn)] // version/layer/mode unreachable!()
    pub fn try_read<R: Read>(
        reader: &mut Reader<'_, R>,
    ) -> PositionalResult<TryReadFrameHeaderOutcome> {
        let header_word = if let Some(header_word) = try_read_next_header_word(reader)? {
            header_word
        } else {
            return Ok(Ok(None));
        };

        let version = version_from_header_word(header_word).expect("valid version");

        let sample_rate_hz =
            sample_rate_hz_from_bits(version, sample_rate_bits_from_header_word(header_word));
        debug_assert!(sample_rate_hz > 0);

        let layer = layer_from_header_word(header_word).expect("valid layer");

        let bitrate_bps =
            bitrate_bps_from_bits(version, layer, bitrate_bits_from_header_word(header_word));

        let sample_count = sample_count(version, layer);

        let mode = mode_from_header_word(header_word);

        let padding = (header_word >> 9) & 0b1;

        let frame_size = if layer == Layer::Layer1 {
            (12 * bitrate_bps / u32::from(sample_rate_hz) + padding) * 4
        } else {
            u32::from(sample_count) * (bitrate_bps / 8) / u32::from(sample_rate_hz) + padding
        };
        debug_assert!(frame_size <= u16::MAX.into());
        let frame_size = frame_size as u16;

        Ok(Ok(Some(Self {
            version,
            layer,
            mode,
            sample_rate_hz,
            sample_count,
            bitrate_bps: (bitrate_bps > 0).then(|| bitrate_bps),
            frame_size: (frame_size > 0).then(|| frame_size),
        })))
    }
}
