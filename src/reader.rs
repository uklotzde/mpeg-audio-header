use std::{
    io::{self, prelude::*},
    time::Duration,
};

use crate::{
    error::{Error, PositionalError},
    PositionalResult,
};

/// Position within a readable source
#[derive(Debug, Clone)]
pub struct ReadPosition {
    pub(crate) byte_offset: u64,
    pub(crate) duration: Duration,
}

impl ReadPosition {
    pub(crate) const fn new() -> Self {
        Self {
            byte_offset: 0,
            duration: Duration::ZERO,
        }
    }

    /// The number of bytes that have been consumed
    #[must_use]
    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }

    /// The accumulated duration since the start of the stream
    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }
}

pub(crate) struct Reader<'r, T> {
    reader: &'r mut T,
    position: ReadPosition,
}

impl<'r, T: Read> Reader<'r, T> {
    #[must_use]
    pub(crate) fn new(reader: &'r mut T) -> Self {
        Reader {
            reader,
            position: ReadPosition::new(),
        }
    }

    fn read_exact(&mut self, buffer: &mut [u8]) -> PositionalResult<()> {
        self.reader
            .read_exact(buffer)
            .map(|()| {
                self.position.byte_offset += buffer.len() as u64;
            })
            .map_err(|e| self.positional_error(e.into()))
    }

    pub(crate) fn try_read_exact_until_eof(&mut self, buffer: &mut [u8]) -> PositionalResult<bool> {
        self.read_exact(buffer).map(|()| true).or_else(|err| {
            if err.is_unexpected_eof() {
                Ok(false)
            } else {
                Err(err)
            }
        })
    }

    fn skip(&mut self, max_bytes: u64) -> PositionalResult<u64> {
        match io::copy(&mut self.reader.take(max_bytes), &mut io::sink()) {
            Err(e) => Err(self.positional_error(e.into())),
            Ok(num_bytes_skipped) => {
                debug_assert!(num_bytes_skipped <= max_bytes);
                self.position.byte_offset += num_bytes_skipped;
                Ok(num_bytes_skipped)
            }
        }
    }

    pub(crate) fn try_skip_exact_until_eof(&mut self, num_bytes: u64) -> PositionalResult<bool> {
        match self.skip(num_bytes) {
            Ok(skipped_bytes) => {
                debug_assert!(skipped_bytes <= num_bytes);
                Ok(skipped_bytes == num_bytes)
            }
            Err(err) => {
                if err.is_unexpected_eof() {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
        }
    }

    #[must_use]
    pub(crate) fn position(&self) -> &ReadPosition {
        &self.position
    }

    pub(crate) fn add_duration(&mut self, duration: Duration) {
        self.position.duration += duration;
    }

    #[must_use]
    pub(crate) fn positional_error(&self, source: Error) -> PositionalError {
        let Self { position, .. } = self;
        PositionalError {
            source,
            position: position.clone(),
        }
    }
}
