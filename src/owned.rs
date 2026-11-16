use crate::{Diagnostic, ExtractionError, Extractor, Fields, PackedDescriptor, Prefix, Status};
use std::sync::atomic::AtomicBool;

/// Owned packed metadata produced by the allocating convenience API.
///
/// Values are descriptive text, not a typed representation of raw format values.
/// The allocation contains only output and survives source retirement.
#[derive(Debug)]
pub struct OwnedMetadata {
    bytes: Vec<u8>,
    descriptor: PackedDescriptor,
}
impl OwnedMetadata {
    /// Extraction coverage/result classification.
    #[must_use]
    pub fn status(&self) -> Status {
        self.descriptor.status()
    }
    /// Bounded diagnostic explaining a non-complete result.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        self.descriptor.diagnostic()
    }
    /// Iterate structured directory/name/value fields.
    #[must_use]
    pub fn fields(&self) -> Fields<'_> {
        // The bytes are private and unchanged since successful extraction.
        self.descriptor
            .fields(&self.bytes)
            .expect("owned packed output remains valid")
    }
}

/// Extract metadata using default bounds and owned scratch/output allocations.
///
/// Values are human-readable descriptions. Malformed, unsupported and resource
/// results are reported by OwnedMetadata status/diagnostic, with no error sentinel.
/// The zero-allocation alternative is Extractor with caller-owned storage.
///
/// # Errors
/// Rejects invalid storage/admission. Total borrowed input bytes do not consume
/// scratch/output capacity; actual traversal is constrained by the work limit.
pub fn extract(bytes: &[u8]) -> Result<OwnedMetadata, ExtractionError> {
    let extractor = Extractor::default();
    let plan = extractor.plan(bytes.len())?;
    let mut scratch = vec![0; plan.workspace_bytes];
    let mut output = vec![0; plan.output_bytes];
    let parts = [bytes];
    let result = extractor.extract(
        Prefix::new(&parts, bytes.len())?,
        &mut scratch,
        &mut output,
        &AtomicBool::new(false),
    )?;
    let descriptor = result.descriptor();
    output.truncate(descriptor.used_output_bytes());
    Ok(OwnedMetadata {
        bytes: output,
        descriptor,
    })
}
