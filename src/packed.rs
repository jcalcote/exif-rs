use crate::{Control, Diagnostic, Failure, MAX_TEXT_BYTES, ParseResult as Result, Status};
use crate::{ExtractionError, MAX_ENTRIES, MAX_OUTPUT_BYTES};
use std::fmt::{self, Write};

pub(crate) const INDEX_BYTES: usize = 12;

/// A directory/name/value view into detached output storage.
///
/// Values are descriptions, not reconstructed typed raw values. Repeated names
/// in the same directory retain the last value; directory occurrences are not
/// distinguished by this first release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Field<'a> {
    /// Human-readable directory name.
    pub directory: &'a str,
    /// Human-readable field name.
    pub name: &'a str,
    /// Descriptive UTF-8 text.
    pub value: &'a str,
}

/// Extraction view borrowing only the caller's output arena.
#[derive(Debug)]
pub struct Extraction<'a> {
    bytes: &'a [u8],
    descriptor: PackedDescriptor,
}
impl Extraction<'_> {
    /// Coverage/result classification.
    #[must_use]
    pub fn status(&self) -> Status {
        self.descriptor.status
    }
    /// Bounded diagnostic category.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        self.descriptor.diagnostic
    }
    /// Copy output ownership metadata without retaining a source/parser borrow.
    #[must_use]
    pub fn descriptor(&self) -> PackedDescriptor {
        self.descriptor
    }
    /// Iterate structured fields; strings borrow only output.
    #[must_use]
    pub fn fields(&self) -> Fields<'_> {
        Fields {
            bytes: self.bytes,
            count: self.descriptor.count,
            next: 0,
        }
    }
    /// Occupied bytes including the complete reserved index.
    #[must_use]
    pub fn used_output_bytes(&self) -> usize {
        self.descriptor.used
    }
}
/// Copyable private descriptor for an owned arena, without self-reference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackedDescriptor {
    count: usize,
    used: usize,
    index_bytes: usize,
    status: Status,
    diagnostic: Diagnostic,
}
impl PackedDescriptor {
    /// Coverage/result classification.
    #[must_use]
    pub fn status(self) -> Status {
        self.status
    }
    /// Bounded diagnostic category.
    #[must_use]
    pub fn diagnostic(self) -> Diagnostic {
        self.diagnostic
    }
    /// Occupied bytes including the complete reserved index.
    #[must_use]
    pub fn used_output_bytes(self) -> usize {
        self.used
    }
    /// Validate mutated/supplied storage before creating an iterator.
    ///
    /// Inspects at most 512 records/256 KiB and 130,816 pair comparisons, without
    /// allocation. Structural validity does not establish semantic provenance.
    ///
    /// # Errors
    /// Rejects short storage, invalid offsets, overlaps, reserved bytes or UTF-8.
    pub fn fields(self, output: &[u8]) -> std::result::Result<Fields<'_>, ExtractionError> {
        let invalid = ExtractionError::InvalidPackedOutput;
        if self.used > MAX_OUTPUT_BYTES
            || self.count > MAX_ENTRIES
            || self.index_bytes > MAX_ENTRIES * INDEX_BYTES
            || self.index_bytes % INDEX_BYTES != 0
            || self.count * INDEX_BYTES > self.index_bytes
            || self.index_bytes > self.used
        {
            return Err(invalid);
        }
        if output.len() < self.used {
            return Err(ExtractionError::InsufficientStorage);
        }
        let bytes = &output[..self.used];
        for n in 0..self.count {
            let (at, d, k, v) = index(bytes, n);
            let end = at.checked_add(d + k + v).ok_or(invalid)?;
            if at < self.index_bytes
                || end > self.used
                || [d, k, v].iter().any(|n| *n > MAX_TEXT_BYTES)
                || bytes[n * INDEX_BYTES + 10..n * INDEX_BYTES + 12] != [0, 0]
            {
                return Err(invalid);
            }
            for old in 0..n {
                let (oa, od, ok, ov) = index(bytes, old);
                if at < oa + od + ok + ov && oa < end {
                    return Err(invalid);
                }
            }
            for part in [
                &bytes[at..at + d],
                &bytes[at + d..at + d + k],
                &bytes[at + d + k..end],
            ] {
                std::str::from_utf8(part).map_err(|_| invalid)?;
            }
        }
        Ok(Fields {
            bytes,
            count: self.count,
            next: 0,
        })
    }
}
/// Iterator over already validated detached structured text fields.
#[derive(Debug)]
pub struct Fields<'a> {
    bytes: &'a [u8],
    count: usize,
    next: usize,
}
impl<'a> Iterator for Fields<'a> {
    type Item = Field<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next == self.count {
            return None;
        }
        let (at, d, k, v) = index(self.bytes, self.next);
        self.next += 1;
        Some(Field {
            directory: std::str::from_utf8(&self.bytes[at..at + d]).ok()?,
            name: std::str::from_utf8(&self.bytes[at + d..at + d + k]).ok()?,
            value: std::str::from_utf8(&self.bytes[at + d + k..at + d + k + v]).ok()?,
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.count - self.next;
        (left, Some(left))
    }
}
impl ExactSizeIterator for Fields<'_> {}
fn index(bytes: &[u8], entry: usize) -> (usize, usize, usize, usize) {
    let at = entry * INDEX_BYTES;
    (
        u32::from_le_bytes(bytes[at..at + 4].try_into().expect("validated index")) as usize,
        usize::from(u16::from_le_bytes([bytes[at + 4], bytes[at + 5]])),
        usize::from(u16::from_le_bytes([bytes[at + 6], bytes[at + 7]])),
        usize::from(u16::from_le_bytes([bytes[at + 8], bytes[at + 9]])),
    )
}
fn set_index(bytes: &mut [u8], entry: usize, offset: usize, dir: usize, name: usize, value: usize) {
    let at = entry * INDEX_BYTES;
    bytes[at..at + 4]
        .copy_from_slice(&u32::try_from(offset).expect("bounded output").to_le_bytes());
    for (pos, n) in [(4, dir), (6, name), (8, value)] {
        bytes[at + pos..at + pos + 2]
            .copy_from_slice(&u16::try_from(n).expect("bounded text").to_le_bytes());
    }
    bytes[at + 10..at + 12].fill(0);
}

pub(crate) struct Writer<'a> {
    bytes: &'a mut [u8],
    capacity: usize,
    count: usize,
    used: usize,
}
impl<'a> Writer<'a> {
    pub(crate) fn new(bytes: &'a mut [u8], capacity: usize) -> Self {
        Self {
            bytes,
            capacity,
            count: 0,
            used: capacity * INDEX_BYTES,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.count = 0;
        self.used = self.capacity * INDEX_BYTES;
    }
    pub(crate) fn finish(self, status: Status, diagnostic: Diagnostic) -> Extraction<'a> {
        Extraction {
            bytes: self.bytes,
            descriptor: PackedDescriptor {
                count: self.count,
                used: self.used,
                index_bytes: self.capacity * INDEX_BYTES,
                status,
                diagnostic,
            },
        }
    }
    fn matches(&self, i: usize, directory: &str, name: &str) -> bool {
        let (at, d, k, _) = index(self.bytes, i);
        self.bytes[at..at + d] == *directory.as_bytes()
            && self.bytes[at + d..at + d + k] == *name.as_bytes()
    }
    pub(crate) fn has(&self, directory: &str, name: &str, c: &mut Control<'_>) -> Result<bool> {
        for i in 0..self.count {
            c.step()?;
            c.work(directory.len() + name.len())?;
            if self.matches(i, directory, name) {
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(crate) fn put(
        &mut self,
        directory: &str,
        name: &str,
        value: &str,
        c: &mut Control<'_>,
    ) -> Result<()> {
        c.step()?;
        if [directory, name, value]
            .iter()
            .any(|s| s.len() > MAX_TEXT_BYTES)
        {
            return Err(Failure::Limit(Diagnostic::TextLimit));
        }
        let mut found = None;
        for i in 0..self.count {
            c.step()?;
            c.work(directory.len() + name.len())?;
            if self.matches(i, directory, name) {
                let (at, d, k, v) = index(self.bytes, i);
                found = Some((i, at, d + k + v));
                break;
            }
        }
        let removed = found.map_or(0, |(_, _, size)| size);
        let needed = directory.len() + name.len() + value.len();
        if needed > self.bytes.len() - (self.used - removed) {
            return Err(Failure::Limit(Diagnostic::OutputLimit));
        }
        c.work(needed + found.map_or(0, |(_, at, size)| self.used - at - size))?;
        let entry = if let Some((i, at, size)) = found {
            self.bytes.copy_within(at + size..self.used, at);
            self.used -= size;
            for other in 0..self.count {
                c.step()?;
                let (oa, d, k, v) = index(self.bytes, other);
                if oa > at {
                    set_index(self.bytes, other, oa - size, d, k, v);
                }
            }
            i
        } else {
            if self.count == self.capacity {
                return Err(Failure::Limit(Diagnostic::EntryLimit));
            }
            self.count += 1;
            self.count - 1
        };
        let mut at = self.used;
        for part in [directory, name, value] {
            self.bytes[at..at + part.len()].copy_from_slice(part.as_bytes());
            at += part.len();
        }
        set_index(
            self.bytes,
            entry,
            self.used,
            directory.len(),
            name.len(),
            value.len(),
        );
        self.used += needed;
        Ok(())
    }
    pub(crate) fn text(
        &mut self,
        directory: &str,
        name: &str,
        value: &Text,
        c: &mut Control<'_>,
    ) -> Result<()> {
        if value.overflow {
            return Err(Failure::Limit(Diagnostic::TextLimit));
        }
        self.put(directory, name, value.as_str(), c)
    }
    pub(crate) fn number(
        &mut self,
        directory: &str,
        name: &str,
        value: impl fmt::Display,
        c: &mut Control<'_>,
    ) -> Result<()> {
        let mut text = Text::new();
        let _ = write!(text, "{value}");
        self.text(directory, name, &text, c)
    }
}

pub(crate) struct Text {
    bytes: [u8; MAX_TEXT_BYTES],
    len: usize,
    overflow: bool,
}
impl Text {
    pub(crate) fn new() -> Self {
        Self {
            bytes: [0; MAX_TEXT_BYTES],
            len: 0,
            overflow: false,
        }
    }
    pub(crate) fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
    pub(crate) fn trim_decimal(&mut self) {
        if let Some(point) = self.bytes[..self.len].iter().position(|byte| *byte == b'.') {
            while self.len > point + 1 && self.bytes[self.len - 1] == b'0' {
                self.len -= 1;
            }
            if self.len == point + 1 {
                self.len -= 1;
            }
        }
    }
    pub(crate) fn mark_overflow(&mut self) {
        self.overflow = true;
    }
    pub(crate) fn is_overflow(&self) -> bool {
        self.overflow
    }
    pub(crate) fn push(&mut self, ch: char) {
        if self.overflow || self.len + ch.len_utf8() > MAX_TEXT_BYTES {
            self.overflow = true;
            return;
        }
        let mut encoded = [0u8; 4];
        let s = ch.encode_utf8(&mut encoded);
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
    }
}
impl Write for Text {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for ch in text.chars() {
            self.push(ch);
        }
        if self.overflow {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}
