use crate::{Control, ExtractionError, Failure, MAX_SEGMENTS, ParseResult as Result};

/// Validated borrowed source prefix. Construction and extraction never copy payload.
#[derive(Clone, Copy, Debug)]
pub struct Prefix<'a> {
    parts: &'a [&'a [u8]],
    start: usize,
    len: usize,
}
impl<'a> Prefix<'a> {
    /// Validate exact supplied length, using checked arithmetic.
    ///
    /// # Errors
    /// Rejects inconsistent/overflowing total length or more than MAX_SEGMENTS descriptors.
    pub fn new(
        parts: &'a [&'a [u8]],
        expected_len: usize,
    ) -> std::result::Result<Self, ExtractionError> {
        if parts.len() > MAX_SEGMENTS {
            return Err(ExtractionError::InvalidPrefix);
        }
        let len = checked_length(parts.iter().map(|part| part.len()), expected_len)?;
        Ok(Self {
            parts,
            start: 0,
            len,
        })
    }
    /// Number of original bytes visible through this prefix.
    #[must_use]
    pub fn len(self) -> usize {
        self.len
    }
    /// Whether the prefix is empty.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    pub(crate) fn slice(self, start: usize, len: usize) -> Result<Self> {
        if start > self.len || len > self.len - start {
            return Err(Failure::Malformed);
        }
        Ok(Self {
            parts: self.parts,
            start: self.start + start,
            len,
        })
    }
    pub(crate) fn read(self, offset: usize, out: &mut [u8], c: &mut Control<'_>) -> Result<()> {
        if offset > self.len || out.len() > self.len - offset {
            return Err(Failure::Malformed);
        }
        let mut skip = self.start + offset;
        let mut copied = 0;
        for part in self.parts {
            c.step()?;
            if skip >= part.len() {
                skip -= part.len();
                continue;
            }
            let take = (part.len() - skip).min(out.len() - copied);
            out[copied..copied + take].copy_from_slice(&part[skip..skip + take]);
            copied += take;
            if copied == out.len() {
                return Ok(());
            }
            skip = 0;
        }
        if out.is_empty() {
            Ok(())
        } else {
            Err(Failure::Malformed)
        }
    }
    pub(crate) fn array<const N: usize>(
        self,
        offset: usize,
        c: &mut Control<'_>,
    ) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read(offset, &mut bytes, c)?;
        Ok(bytes)
    }
    pub(crate) fn byte(self, offset: usize, c: &mut Control<'_>) -> Result<u8> {
        Ok(self.array::<1>(offset, c)?[0])
    }
    pub(crate) fn u16(self, offset: usize, little: bool, c: &mut Control<'_>) -> Result<u16> {
        let bytes = self.array(offset, c)?;
        Ok(if little {
            u16::from_le_bytes(bytes)
        } else {
            u16::from_be_bytes(bytes)
        })
    }
    pub(crate) fn u32(self, offset: usize, little: bool, c: &mut Control<'_>) -> Result<u32> {
        let bytes = self.array(offset, c)?;
        Ok(if little {
            u32::from_le_bytes(bytes)
        } else {
            u32::from_be_bytes(bytes)
        })
    }
    pub(crate) fn starts(self, bytes: &[u8], c: &mut Control<'_>) -> Result<bool> {
        if self.len < bytes.len() {
            return Ok(false);
        }
        for (i, byte) in bytes.iter().enumerate() {
            if self.byte(i, c)? != *byte {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Decode text into the fixed UTF-8 text envelope; overflow is explicit.
pub(crate) fn text(
    input: Prefix<'_>,
    offset: usize,
    len: usize,
    latin1: bool,
    nul: bool,
    c: &mut Control<'_>,
) -> Result<crate::packed::Text> {
    let input = input.slice(offset, len)?;
    let mut result = crate::packed::Text::new();
    let mut at = 0;
    while at < len {
        c.step()?;
        let first = input.byte(at, c)?;
        if nul && first == 0 {
            break;
        }
        if latin1 || first < 128 {
            result.push(char::from(first));
            at += 1;
        } else {
            let width = match first {
                0xc2..=0xdf => 2,
                0xe0..=0xef => 3,
                0xf0..=0xf4 => 4,
                _ => 1,
            };
            let mut encoded = [0; 4];
            let take = width.min(len - at);
            input.read(at, &mut encoded[..take], c)?;
            let ch = std::str::from_utf8(&encoded[..take])
                .ok()
                .and_then(|s| s.chars().next());
            if let Some(ch) = ch {
                result.push(ch);
                at += take;
            } else {
                result.push('\u{fffd}');
                at += 1;
            }
        }
        if result.is_overflow() {
            return Err(Failure::Limit(crate::Diagnostic::TextLimit));
        }
    }
    Ok(result)
}

// Shared with constructor tests so overflow is exercised without forging slices.
pub(crate) fn checked_length(
    lengths: impl Iterator<Item = usize>,
    expected: usize,
) -> std::result::Result<usize, ExtractionError> {
    let mut len = 0usize;
    for next in lengths {
        len = len
            .checked_add(next)
            .ok_or(ExtractionError::InvalidPrefix)?;
        if len > expected {
            return Err(ExtractionError::InvalidPrefix);
        }
    }
    if len != expected {
        return Err(ExtractionError::InvalidPrefix);
    }
    Ok(len)
}
