// SPDX-FileCopyrightText: The mpeg-audio-header authors
// SPDX-License-Identifier: MPL-2.0

use thiserror::Error;

use crate::ReadPosition;

/// Error enriched with position information
#[derive(Debug, Error)]
#[error("{} at position {:.3} ms (byte offset = {} / 0x{:X})",
        .source, .position.duration.as_secs_f64() * 1000.0, .position.byte_offset, .position.byte_offset)]
pub struct PositionalError {
    #[source]
    pub(crate) source: Error,

    pub(crate) position: ReadPosition,
}

impl PositionalError {
    /// The actual error
    #[must_use]
    pub const fn source(&self) -> &Error {
        &self.source
    }

    /// The last known position where this error occurred
    #[must_use]
    pub const fn position(&self) -> &ReadPosition {
        &self.position
    }
}

impl PositionalError {
    pub(crate) fn is_unexpected_eof(&self) -> bool {
        self.source.is_unexpected_eof()
    }
}

/// Error type
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Unexpected I/O error occurred
    #[error(transparent)]
    IoError(#[from] std::io::Error),

    #[error("frame error: {0}")]
    FrameError(String),
}

impl Error {
    fn is_unexpected_eof(&self) -> bool {
        match self {
            Self::IoError(err) => {
                matches!(err.kind(), std::io::ErrorKind::UnexpectedEof)
            }
            _ => false,
        }
    }
}
