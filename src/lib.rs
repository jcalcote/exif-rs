//! Bounded metadata extraction from immutable segmented input.
//!
//! Low-level [`Extractor::extract`], iteration and descriptor reborrow allocate
//! no heap. The free [`extract`] convenience function allocates owned storage. Parsing performs no I/O and creates no execution pool.
//! Values are human descriptions; this release does not expose typed raw values.
//!
//! ```
//! let input = [0xff, 0xd8, 0xff, 0xd9];
//! let metadata = exif_inspect::extract(&input)?;
//! for field in metadata.fields() {
//!     println!("{} / {}: {}", field.directory, field.name, field.value);
//! }
//! # Ok::<(), exif_inspect::ExtractionError>(())
//! ```
//!
//! To plan caller-owned storage, use [`Extractor::plan`], [`Prefix::new`] and
//! [`Extractor::extract`]. The `arenas` example demonstrates segmented input and
//! a detached [`PackedDescriptor`]. Parsing is limited by [`MAX_STEPS`],
//! [`MAX_NESTING`] and [`MAX_TEXT_BYTES`]; overflow is always explicit.
#![cfg_attr(not(test), forbid(unsafe_code))]

mod bmff;
mod carriers;
mod icc;
mod images;
mod input;
mod iptc;
mod packed;
mod riff;
mod tables;
mod tiff;

use std::sync::atomic::{AtomicBool, Ordering};

pub use input::Prefix;
pub use packed::{Extraction, Field, Fields, PackedDescriptor};
mod owned;
pub use owned::{OwnedMetadata, extract};

/// Maximum borrowed source segment descriptors. Total byte length is checked, not capped.
pub const MAX_SEGMENTS: usize = 4096;
/// Maximum UTF-8 bytes in each directory, name or descriptive value.
/// Exceeding this limit produces ResourceLimit/TextLimit, never silent truncation.
pub const MAX_TEXT_BYTES: usize = 1024;
/// Maximum configured retained fields.
pub const MAX_ENTRIES: usize = 512;
/// Maximum configured packed output, including its fixed field index.
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;
/// Complete caller-provided scratch, including fixed traversal state.
pub const SCRATCH_BYTES: usize = 128 * 1024;
/// Maximum nested container/IFD depth.
pub const MAX_NESTING: usize = 32;
/// Maximum logical parse, lookup and formatting operations per call.
pub const MAX_STEPS: u64 = 1_000_000;

/// Enforceable output limits; callers may select lower ceilings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    /// Maximum distinct retained fields, including the fixed index capacity.
    pub max_entries: usize,
    /// Complete packed allocation capacity, including the fixed index.
    pub output_bytes: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: MAX_ENTRIES,
            output_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

/// Complete per-call storage inventory; source backings remain caller-owned.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Plan {
    /// Exact admitted original input length.
    pub input_bytes: usize,
    /// Extra payload copy; zero for the segmented reader.
    pub contiguous_copy_bytes: usize,
    /// Scratch allocation, including traversal state.
    pub workspace_bytes: usize,
    /// Complete packed output, including the index below.
    pub output_bytes: usize,
    /// Fixed index bytes contained within `output_bytes`, never additional storage.
    pub fixed_index_bytes: usize,
    /// Maximum retained fields.
    pub max_entries: usize,
}

/// Extraction status. Only `Complete` denotes full coverage of encountered data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    /// All encountered metadata belonged to implemented supported paths.
    Complete,
    /// Unknown format or malformed/truncated source prevented extraction.
    ReaderError,
    /// Some encountered metadata requires a later supported-format checkpoint.
    Partial,
    /// A workspace/output/depth/operation ceiling was exhausted.
    ResourceLimit,
}

/// Diagnostic category without customer payload or unbounded diagnostic storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Diagnostic {
    /// No extraction error.
    None,
    /// The format was not recognized.
    UnknownFormat,
    /// Input structure or offsets were invalid/incomplete.
    MalformedInput,
    /// A recognized carrier or metadata section is not implemented yet.
    UnsupportedMetadata,
    /// Distinct output field capacity exhausted.
    EntryLimit,
    /// Packed output byte capacity exhausted.
    OutputLimit,
    /// A directory, name or descriptive value exceeded MAX_TEXT_BYTES.
    TextLimit,
    /// Nested container/IFD depth exhausted.
    NestingLimit,
    /// Parse, lookup or formatting operation allowance exhausted.
    StepLimit,
}

/// Invalid configuration/storage, or cancellation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtractionError {
    /// Limits exceed supported ceilings or cannot contain their fixed index.
    InvalidLimits,
    /// Segmented input length is inconsistent or exceeds the prefix ceiling.
    InvalidPrefix,
    /// Caller storage does not cover the complete plan.
    InsufficientStorage,
    /// Packed storage no longer matches a valid bounded output representation.
    InvalidPackedOutput,
    /// Cancellation observed before or during extraction.
    Cancelled,
}
impl std::fmt::Display for ExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "metadata extraction: {self:?}")
    }
}
impl std::error::Error for ExtractionError {}

/// Immutable, allocation-free extractor policy. Shared execution belongs to caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extractor {
    limits: Limits,
}
impl Extractor {
    /// Construct an extractor without allocating compiled state.
    ///
    /// # Errors
    /// Rejects ceilings above the supported maxima or an underfunded fixed index.
    pub fn new(limits: Limits) -> Result<Self, ExtractionError> {
        if limits.max_entries > MAX_ENTRIES
            || limits.output_bytes > MAX_OUTPUT_BYTES
            || limits
                .max_entries
                .checked_mul(packed::INDEX_BYTES)
                .is_none_or(|index| index > limits.output_bytes)
        {
            return Err(ExtractionError::InvalidLimits);
        }
        Ok(Self { limits })
    }

    /// Compute exact scratch and output admission for this input.
    ///
    /// # Errors
    /// No input bytes are silently discarded. The result accounts for borrowed
    /// bytes independently of fixed scratch/output allocations.
    pub fn plan(&self, input_len: usize) -> Result<Plan, ExtractionError> {
        Ok(Plan {
            input_bytes: input_len,
            contiguous_copy_bytes: 0,
            workspace_bytes: SCRATCH_BYTES,
            output_bytes: self.limits.output_bytes,
            fixed_index_bytes: self.limits.max_entries * packed::INDEX_BYTES,
            max_entries: self.limits.max_entries,
        })
    }

    /// Extract into caller-provided storage. Result borrows only packed output.
    ///
    /// On `ResourceLimit` fields are empty: callers inspect the explicit
    /// status and the diagnostic. Partial exposes supported fields with an explicit
    /// coverage diagnostic; it must not be presented as complete extraction.
    /// All source and workspace borrows retire before this synchronous call returns.
    ///
    /// # Errors
    /// Rejects insufficient arenas or cancellation.
    pub fn extract<'output>(
        &self,
        prefix: Prefix<'_>,
        workspace: &mut [u8],
        output: &'output mut [u8],
        cancel: &AtomicBool,
    ) -> Result<Extraction<'output>, ExtractionError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractionError::Cancelled);
        }
        if workspace.len() < SCRATCH_BYTES || output.len() < self.limits.output_bytes {
            return Err(ExtractionError::InsufficientStorage);
        }
        let mut writer = packed::Writer::new(
            &mut output[..self.limits.output_bytes],
            self.limits.max_entries,
        );
        let mut control = Control {
            cancel,
            remaining: MAX_STEPS,
            partial: false,
        };
        let result = images::extract(
            prefix,
            &mut workspace[..SCRATCH_BYTES],
            &mut writer,
            &mut control,
        );
        match result {
            Ok(()) if control.partial => {
                Ok(writer.finish(Status::Partial, Diagnostic::UnsupportedMetadata))
            }
            Ok(()) => Ok(writer.finish(Status::Complete, Diagnostic::None)),
            Err(Failure::Cancelled) => Err(ExtractionError::Cancelled),
            Err(Failure::Limit(diagnostic)) => {
                writer.clear();
                Ok(writer.finish(Status::ResourceLimit, diagnostic))
            }
            Err(Failure::Malformed) => {
                writer.clear();
                Ok(writer.finish(Status::ReaderError, Diagnostic::MalformedInput))
            }
            Err(Failure::Unknown) => {
                Ok(writer.finish(Status::ReaderError, Diagnostic::UnknownFormat))
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Failure {
    Malformed,
    Unknown,
    Limit(Diagnostic),
    Cancelled,
}
type ParseResult<T> = std::result::Result<T, Failure>;
struct Control<'a> {
    cancel: &'a AtomicBool,
    remaining: u64,
    partial: bool,
}
impl Control<'_> {
    fn step(&mut self) -> ParseResult<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Failure::Cancelled);
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(Failure::Limit(Diagnostic::StepLimit))?;
        Ok(())
    }
    fn work(&mut self, bytes: usize) -> ParseResult<()> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Failure::Cancelled);
        }
        self.remaining = self
            .remaining
            .checked_sub(bytes as u64)
            .ok_or(Failure::Limit(Diagnostic::StepLimit))?;
        Ok(())
    }
    fn depth(&mut self, depth: usize) -> ParseResult<()> {
        self.step()?;
        if depth >= MAX_NESTING {
            return Err(Failure::Limit(Diagnostic::NestingLimit));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod allocation_tests;
