use mpeg_audio_header::{Header, HeaderSource, Layer, Mode, Version};

#[test]
fn public_api() {
    // Verify that all types are accessible
    let _header = Header {
        source: HeaderSource::MpegFrameHeaders,
        layer: Some(Layer::Layer1),
        mode: Some(Mode::DualChannel),
        version: Some(Version::Mpeg1),
        avg_bitrate_bps: None,
        min_channel_count: Default::default(),
        max_channel_count: Default::default(),
        min_sample_rate_hz: Default::default(),
        max_sample_rate_hz: Default::default(),
        avg_sample_rate_hz: None,
        total_duration: Default::default(),
        total_sample_count: Default::default(),
    };
}
