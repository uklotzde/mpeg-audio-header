use walkdir::{DirEntry, WalkDir};

use super::*;

const TEST_DATA_DIR: &str = "test-data/";

#[allow(clippy::case_sensitive_file_extension_comparisons)]
fn is_supported_file_extension(entry: &DirEntry) -> bool {
    if entry.file_type().is_dir() {
        return true;
    }
    // symlinks are resolved by follow_symlinks = true
    debug_assert!(entry.file_type().is_file());
    entry.file_name().to_str().map_or(false, |file_name| {
        let file_name = file_name.to_lowercase();
        file_name.ends_with(".mp3") || file_name.ends_with(".mp2") || file_name.ends_with(".mp1")
    })
}

fn filter_expected_errors(
    _path_suffix: &str,
    _parse_mode: ParseMode,
    err: PositionalError,
) -> PositionalResult<Option<Header>> {
    // match path_suffix {
    //     _ => Err(err),
    // }
    Err(err)
}

fn check_header(path_suffix: &str, parse_mode: ParseMode, header: Header) -> Header {
    match path_suffix {
        "minimp3/performance/MEANDR90.mp3" => {
            assert_eq!(Duration::from_millis(240), header.total_duration);
        }
        "minimp3/performance/MEANDR_PHASE0.mp3" => {
            assert_eq!(Duration::from_millis(100_032), header.total_duration);
        }
        "minimp3/performance/MEANDR_PHASE90.mp3" | "minimp3/performance/noise_meandr.mp3" => {
            assert_eq!(Duration::from_millis(10_032), header.total_duration);
        }
        "minimp3/performance/MIPSTest.mp3" => {
            assert_eq!(Duration::from_millis(8_208), header.total_duration);
        }
        "minimp3/fuzz/l3-compl-cut.mp3" => {
            assert_eq!(Duration::from_millis(24), header.total_duration);
        }
        "mp3-duration/CBR320.mp3" | "mp3-duration/VBR0.mp3" => {
            if matches!(parse_mode, ParseMode::PreferVbrHeaders) {
                assert_eq!(HeaderSource::XingHeader, header.source);
                assert_eq!(Duration::from_nanos(398_341_224_489), header.total_duration);
            } else {
                assert_eq!(HeaderSource::MpegFrameHeaders, header.source);
                assert_eq!(Duration::from_nanos(398_341_209_552), header.total_duration);
            }
        }
        "mp3-duration/ID3v1.mp3"
        | "mp3-duration/ID3v2.mp3"
        | "mp3-duration/ID3v2WithBadPadding.mp3"
        | "mp3-duration/ID3v2WithImage.mp3"
        | "mp3-duration/APEv2.mp3"
        | "mp3-duration/source.mp3" => {
            assert_eq!(Duration::from_nanos(398_288_964_656), header.total_duration);
        }
        "mp3-duration/MPEGFrameTooShort.mp3" => {
            assert_eq!(Duration::from_nanos(395_519_985_168), header.total_duration);
        }
        "mp3-duration/SineEmptyID3.mp3" => {
            if matches!(parse_mode, ParseMode::PreferVbrHeaders) {
                assert_eq!(HeaderSource::XingHeader, header.source);
                assert_eq!(Duration::from_nanos(1_071_020_408), header.total_duration);
            } else {
                assert_eq!(HeaderSource::MpegFrameHeaders, header.source);
                assert_eq!(Duration::from_nanos(1_071_020_368), header.total_duration);
            }
        }
        "mp3-duration/Truncated.mp3" => {
            assert_eq!(Duration::from_nanos(206_706_931_024), header.total_duration);
        }
        "mp3-duration/VBR9.mp3" => {
            if matches!(parse_mode, ParseMode::PreferVbrHeaders) {
                assert_eq!(HeaderSource::XingHeader, header.source);
                assert_eq!(Duration::from_nanos(398_367_346_938), header.total_duration);
            } else {
                assert_eq!(HeaderSource::MpegFrameHeaders, header.source);
                assert_eq!(Duration::from_nanos(398_367_332_000), header.total_duration);
            }
        }
        "samples.ffmpeg.org/A-codecs/mp1-sample.mp1" => {
            assert_eq!(Duration::from_millis(588), header.total_duration);
        }
        "getID3-testfiles/mp3/VBRI/VBR-10s-44x16x2-q75-19170Hz-random.mp3" => {
            if matches!(parse_mode, ParseMode::PreferVbrHeaders) {
                assert_eq!(HeaderSource::VbriHeader, header.source);
                assert_eq!(Duration::from_nanos(10_083_265_306), header.total_duration);
            } else {
                assert_eq!(HeaderSource::MpegFrameHeaders, header.source);
                assert_eq!(Duration::from_nanos(10_057_142_480), header.total_duration);
            }
        }
        _ => {
            eprintln!("Unchecked result: {:?}", header);
        }
    }
    header
}

fn try_read_header_from_path(
    path: &std::path::Path,
    parse_mode: ParseMode,
) -> anyhow::Result<Option<Header>> {
    let path_suffix = path.to_str().unwrap().strip_prefix(TEST_DATA_DIR).unwrap();
    match Header::read_from_path(path, parse_mode) {
        Ok(header) => Ok(Some(check_header(path_suffix, parse_mode, header))),
        Err(err) => filter_expected_errors(path_suffix, parse_mode, err).map_err(Into::into),
    }
}

#[test]
fn read_all() -> anyhow::Result<()> {
    if let Err(err) = std::fs::read_dir(TEST_DATA_DIR) {
        if matches!(err.kind(), std::io::ErrorKind::NotFound) {
            eprintln!("No test data available");
            // Skip this test if not test data is available
            return Ok(());
        }
    }
    for entry in WalkDir::new(TEST_DATA_DIR)
        .follow_links(true)
        .into_iter()
        .filter_entry(is_supported_file_extension)
    {
        let entry = entry?;
        if entry.file_type().is_dir() {
            // Skip directories
            continue;
        }
        // symlinks are resolved by follow_symlinks = true
        debug_assert!(entry.file_type().is_file());
        let file_path = entry.path();
        println!("Reading file: {}", file_path.display());
        try_read_header_from_path(entry.path(), ParseMode::PreferVbrHeaders)?;
        try_read_header_from_path(entry.path(), ParseMode::IgnoreVbrHeaders)?;
    }

    Ok(())
}
