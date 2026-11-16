// Reader structure, tag dispatch and descriptions adapted from metadata-extractor
// 2.19.0. Copyright 2002-2019 Drew Noakes and contributors, Apache-2.0.
use crate::packed::{Text, Writer};
use crate::{Control, Diagnostic, Failure, ParseResult as Result, Prefix};
use crate::{icc, input, iptc, tables};
use std::fmt::Write;

#[derive(Clone, Copy)]
enum Kind {
    Ifd0,
    Panasonic,
    Sub,
    Thumbnail,
    Gps,
    Interop,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Ifd0 => "Exif IFD0",
            Self::Panasonic => "PanasonicRaw Exif IFD0",
            Self::Sub => "Exif SubIFD",
            Self::Thumbnail => "Exif Thumbnail",
            Self::Gps => "GPS",
            Self::Interop => "Interoperability",
        }
    }
}
#[derive(Clone, Copy)]
struct Value<'a> {
    data: Prefix<'a>,
    typ: u16,
    count: usize,
    offset: usize,
    little: bool,
}
impl<'a> Value<'a> {
    fn span(self) -> Result<Prefix<'a>> {
        let unit = match self.typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => return Err(Failure::Malformed),
        };
        self.data.slice(
            self.offset,
            self.count.checked_mul(unit).ok_or(Failure::Malformed)?,
        )
    }
    fn number(self, n: usize, c: &mut Control<'_>) -> Result<f64> {
        if n >= self.count {
            return Err(Failure::Malformed);
        }
        let at = self.offset;
        Ok(match self.typ {
            1 | 7 => f64::from(self.data.byte(at + n, c)?),
            3 => f64::from(self.data.u16(at + 2 * n, self.little, c)?),
            4 => f64::from(self.data.u32(at + 4 * n, self.little, c)?),
            5 | 10 => {
                let a = self.data.u32(at + 8 * n, self.little, c)?;
                let b = self.data.u32(at + 8 * n + 4, self.little, c)?;
                if self.typ == 10 {
                    f64::from(a as i32) / f64::from(b as i32)
                } else {
                    f64::from(a) / f64::from(b)
                }
            }
            6 => f64::from(self.data.byte(at + n, c)? as i8),
            8 => f64::from(self.data.u16(at + 2 * n, self.little, c)? as i16),
            9 => f64::from(self.data.u32(at + 4 * n, self.little, c)? as i32),
            11 => f64::from(f32::from_bits(self.data.u32(at + 4 * n, self.little, c)?)),
            12 => {
                let b = self.data.array(at + 8 * n, c)?;
                if self.little {
                    f64::from_le_bytes(b)
                } else {
                    f64::from_be_bytes(b)
                }
            }
            _ => return Err(Failure::Malformed),
        })
    }
    fn integer(self, c: &mut Control<'_>) -> Result<i64> {
        if self.count == 0 {
            return Err(Failure::Malformed);
        }
        Ok(match self.typ {
            1 | 7 => i64::from(self.data.byte(self.offset, c)?),
            3 => i64::from(self.data.u16(self.offset, self.little, c)?),
            4 => i64::from(self.data.u32(self.offset, self.little, c)?),
            6 => i64::from(self.data.byte(self.offset, c)? as i8),
            8 => i64::from(self.data.u16(self.offset, self.little, c)? as i16),
            9 => i64::from(self.data.u32(self.offset, self.little, c)? as i32),
            _ => return Err(Failure::Malformed),
        })
    }
    fn unsigned(self, c: &mut Control<'_>) -> Result<u32> {
        if self.count != 1 || self.typ != 4 {
            return Err(Failure::Malformed);
        }
        self.data.u32(self.offset, self.little, c)
    }
    fn raw(self, c: &mut Control<'_>) -> Result<Text> {
        if self.typ == 2 {
            return input::text(self.data, self.offset, self.count, false, true, c);
        }
        let mut out = Text::new();
        if self.count > 16 {
            let _ = write!(out, "[{} values]", self.count);
            return Ok(out);
        }
        for n in 0..self.count {
            c.step()?;
            if n > 0 {
                let _ = out.write_str(" ");
            }
            if self.typ == 5 || self.typ == 10 {
                let a = self.data.u32(self.offset + 8 * n, self.little, c)?;
                let b = self.data.u32(self.offset + 8 * n + 4, self.little, c)?;
                let (a, b) = if self.typ == 10 {
                    (i64::from(a as i32), i64::from(b as i32))
                } else {
                    (i64::from(a), i64::from(b))
                };
                if b != 0 && a % b == 0 {
                    let _ = write!(out, "{}", a / b);
                } else {
                    let _ = write!(out, "{a}/{b}");
                }
            } else {
                let _ = write!(out, "{}", self.number(n, c)?);
            }
        }
        Ok(out)
    }
}
struct Reader<'a, 'scratch> {
    data: Prefix<'a>,
    little: bool,
    scratch: &'scratch mut [u8],
    seen: usize,
}
impl<'a> Reader<'a, '_> {
    fn value(&self, entry: usize, c: &mut Control<'_>) -> Result<Value<'a>> {
        let typ = self.data.u16(entry + 2, self.little, c)?;
        let count = self.data.u32(entry + 4, self.little, c)? as usize;
        let unit = match typ {
            1 | 2 | 6 | 7 => 1,
            3 | 8 => 2,
            4 | 9 | 11 => 4,
            5 | 10 | 12 => 8,
            _ => return Err(Failure::Malformed),
        };
        let bytes = count.checked_mul(unit).ok_or(Failure::Malformed)?;
        let offset = if bytes <= 4 {
            entry + 8
        } else {
            self.data.u32(entry + 8, self.little, c)? as usize
        };
        self.data.slice(offset, bytes)?;
        Ok(Value {
            data: self.data,
            typ,
            count,
            offset,
            little: self.little,
        })
    }
    fn find(&self, at: usize, tag: u16, c: &mut Control<'_>) -> Result<Option<Value<'a>>> {
        let count = usize::from(self.data.u16(at, self.little, c)?);
        for i in 0..count {
            c.step()?;
            let entry = at + 2 + 12 * i;
            if self.data.u16(entry, self.little, c)? == tag {
                return self.value(entry, c).map(Some);
            }
        }
        Ok(None)
    }
    fn ifd(
        &mut self,
        at: usize,
        kind: Kind,
        depth: usize,
        w: &mut Writer<'_>,
        c: &mut Control<'_>,
    ) -> Result<()> {
        if at == 0 {
            return Ok(());
        }
        c.depth(depth)?;
        for slot in 0..self.seen {
            c.step()?;
            let i = slot * 4;
            let old = u32::from_le_bytes(
                self.scratch[i..i + 4]
                    .try_into()
                    .map_err(|_| Failure::Malformed)?,
            );
            if old as usize == at {
                return Err(Failure::Malformed);
            }
        }
        if self.seen * 4 + 4 > self.scratch.len() {
            return Err(Failure::Limit(Diagnostic::StepLimit));
        }
        self.scratch[self.seen * 4..self.seen * 4 + 4].copy_from_slice(&at.to_le_bytes()[..4]);
        self.seen += 1;
        let count = usize::from(self.data.u16(at, self.little, c)?);
        self.data.slice(at, 2 + count * 12 + 4)?;
        for i in 0..count {
            c.step()?;
            let entry = at + 2 + i * 12;
            let tag = self.data.u16(entry, self.little, c)?;
            let value = self.value(entry, c)?;
            if tag == 0x83bb {
                let bytes = value.span()?;
                if bytes.starts(&[0x1c], c)? {
                    iptc::extract(bytes, w, c)?;
                } else {
                    c.partial = true;
                }
                continue;
            }
            if tag == 0x8773 {
                icc::extract(value.span()?, w, c)?;
                continue;
            }
            if matches!(kind, Kind::Panasonic) && matches!(tag, 0x0013 | 0x0027 | 0x002e | 0x0119) {
                c.partial = true;
                continue;
            }
            let child = match tag {
                0x8769 => Some(Kind::Sub),
                0x8825 => Some(Kind::Gps),
                0xa005 => Some(Kind::Interop),
                _ => None,
            };
            if let Some(child) = child {
                self.ifd(value.unsigned(c)? as usize, child, depth + 1, w, c)?;
                continue;
            }
            if matches!(
                tag,
                0x014a | 0x02bc | 0x83bb | 0x8649 | 0x8773 | 0x927c | 0xc4a5
            ) {
                c.partial = true;
                continue;
            }
            let mut unknown = Text::new();
            let name = match kind {
                Kind::Gps => tables::gps_name(tag),
                Kind::Panasonic => tables::panasonic_name(tag),
                Kind::Thumbnail if tag == 0x0201 => Some("Thumbnail Offset"),
                Kind::Thumbnail if tag == 0x0202 => Some("Thumbnail Length"),
                _ => tables::exif_name(tag),
            }
            .unwrap_or_else(|| {
                let _ = write!(unknown, "Unknown tag (0x{tag:04x})");
                unknown.as_str()
            });
            let description = self.describe(at, kind, tag, value, c)?;
            if let Some(description) = description {
                w.text(kind.name(), name, &description, c)?;
            }
        }
        let next = self.data.u32(at + 2 + count * 12, self.little, c)? as usize;
        if next != 0 {
            self.ifd(next, Kind::Thumbnail, depth + 1, w, c)?;
        }
        Ok(())
    }
    // One dispatch preserves the pinned descriptor boundary and fallback.
    #[allow(clippy::too_many_lines)]
    fn describe(
        &self,
        at: usize,
        kind: Kind,
        tag: u16,
        value: Value<'_>,
        c: &mut Control<'_>,
    ) -> Result<Option<Text>> {
        let mut out = Text::new();
        if matches!(kind, Kind::Panasonic) && tag != 0x0112 {
            if tag == 1 {
                c.partial = true;
                return Ok(None);
            }
            return value.raw(c).map(Some);
        }
        if matches!(kind, Kind::Gps) {
            match tag {
                0 => {
                    for i in 0..value.count.min(4) {
                        if i > 0 {
                            let _ = out.write_str(".");
                        }
                        let _ = write!(out, "{}", value.number(i, c)?);
                    }
                }
                1 | 3 => return value.raw(c).map(Some),
                2 | 4 if value.count == 3 => {
                    let ref_tag = if tag == 2 { 1 } else { 3 };
                    let Some(reference) = self.find(at, ref_tag, c)? else {
                        c.partial = true;
                        return Ok(None);
                    };
                    let negative = matches!(reference.raw(c)?.as_str(), "S" | "W");
                    let degrees = value.number(0, c)?;
                    let _ = write!(
                        out,
                        "{}{}° {}' {}\"",
                        if negative { "-" } else { "" },
                        degrees,
                        value.number(1, c)?,
                        value.number(2, c)?
                    );
                }
                5 => {
                    let _ = out.write_str(if value.number(0, c)? == 0.0 {
                        "Sea level"
                    } else {
                        "Below sea level"
                    });
                }
                6 => {
                    let _ = write!(out, "{} metres", value.number(0, c)?);
                }
                7 if value.count == 3 => {
                    let _ = write!(
                        out,
                        "{:02.0}:{:02.0}:{:06.3} UTC",
                        value.number(0, c)?,
                        value.number(1, c)?,
                        value.number(2, c)?
                    );
                }
                _ if tables::gps_indexed_tag(tag) => {
                    let raw = value.integer(c)?;
                    if let Some(name) = tables::gps_indexed(tag, raw) {
                        let _ = out.write_str(name);
                    } else {
                        let _ = write!(out, "Unknown ({raw})");
                    }
                }
                _ if tables::gps_special(tag) => {
                    c.partial = true;
                    return Ok(None);
                }
                _ => return value.raw(c).map(Some),
            }
            return Ok(Some(out));
        }
        match tag {
            0x0112 => {
                let raw = value.integer(c)?;
                let names = [
                    "Top, left side (Horizontal / normal)",
                    "Top, right side (Mirror horizontal)",
                    "Bottom, right side (Rotate 180)",
                    "Bottom, left side (Mirror vertical)",
                    "Left side, top (Mirror horizontal and rotate 270 CW)",
                    "Right side, top (Rotate 90 CW)",
                    "Right side, bottom (Mirror horizontal and rotate 90 CW)",
                    "Left side, bottom (Rotate 270 CW)",
                ];
                if (1..=8).contains(&raw) {
                    let _ = out.write_str(
                        names[usize::try_from(raw - 1).map_err(|_| Failure::Malformed)?],
                    );
                } else {
                    let _ = write!(out, "Unknown ({raw})");
                }
            }
            0x0128 | 0xa210 => {
                let raw = value.integer(c)?;
                let name = match raw {
                    1 => Some("(No unit)"),
                    2 => Some("Inch"),
                    3 => Some("cm"),
                    _ => None,
                };
                if let Some(name) = name {
                    let _ = out.write_str(name);
                } else {
                    let _ = write!(out, "Unknown ({raw})");
                }
            }
            0x011a | 0x011b => {
                let unit = self
                    .find(at, 0x0128, c)?
                    .map(|v| v.integer(c))
                    .transpose()?
                    .unwrap_or(2);
                let _ = write!(
                    out,
                    "{} dots per {}",
                    value.number(0, c)?,
                    match unit {
                        2 => "inch",
                        3 => "cm",
                        _ => "unit",
                    }
                );
            }
            0x829a => {
                let _ = write!(out, "{} sec", value.raw(c)?.as_str());
            }
            0x829d => {
                let _ = write!(out, "f/{:.1}", value.number(0, c)?);
            }
            0x920a => {
                let _ = write!(out, "{} mm", value.number(0, c)?);
            }
            0x0100 | 0x0101 | 0xa002 | 0xa003 => {
                let _ = write!(out, "{} pixels", value.raw(c)?.as_str());
            }
            0x0102 => {
                let _ = write!(out, "{} bits/component/pixel", value.raw(c)?.as_str());
            }
            0x0115 => {
                let _ = write!(out, "{} samples/pixel", value.raw(c)?.as_str());
            }
            0x0116 => {
                let _ = write!(out, "{} rows/strip", value.raw(c)?.as_str());
            }
            0x0117 => {
                let _ = write!(out, "{} bytes", value.raw(c)?.as_str());
            }
            0x8827 => return value.raw(c).map(Some),
            0x9000 | 0xa000 if value.count >= 4 => {
                let major_first = value.data.byte(value.offset, c)?;
                let major_last = value.data.byte(value.offset + 1, c)?;
                if major_first != b'0' && major_first != 0 {
                    out.push(char::from(if major_first < b'0' {
                        major_first + b'0'
                    } else {
                        major_first
                    }));
                }
                out.push(char::from(if major_last < b'0' {
                    major_last + b'0'
                } else {
                    major_last
                }));
                out.push('.');
                let minor_first = value.data.byte(value.offset + 2, c)?;
                let minor_last = value.data.byte(value.offset + 3, c)?;
                out.push(char::from(if minor_first < b'0' {
                    minor_first + b'0'
                } else {
                    minor_first
                }));
                if minor_last != b'0' && minor_last != 0 {
                    out.push(char::from(if minor_last < b'0' {
                        minor_last + b'0'
                    } else {
                        minor_last
                    }));
                }
            }
            0xa001 => {
                let raw = value.integer(c)?;
                let _ = out.write_str(match raw {
                    1 => "sRGB",
                    0xffff => "Undefined",
                    _ => "Unknown",
                });
            }
            _ if tables::exif_indexed_tag(tag) => {
                let raw = value.integer(c)?;
                if let Some(name) = tables::exif_indexed(tag, raw) {
                    let _ = out.write_str(name);
                } else {
                    let _ = write!(out, "Unknown ({raw})");
                }
            }
            _ if tables::exif_special(tag) => {
                c.partial = true;
                return Ok(None);
            }
            _ => return value.raw(c).map(Some),
        }
        Ok(Some(out))
    }
}

pub(crate) fn extract(
    data: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let bytes = data.array::<4>(0, c)?;
    let little = match &bytes {
        b"II\x2a\x00" | b"IIRO" | b"IIRS" | b"II\x55\x00" => true,
        b"MM\x00\x2a" | b"MMOR" => false,
        _ => return Err(Failure::Malformed),
    };
    let at = data.u32(4, little, c)? as usize;
    let mut reader = Reader {
        data,
        little,
        scratch,
        seen: 0,
    };
    let kind = if bytes == *b"II\x55\x00" {
        Kind::Panasonic
    } else {
        Kind::Ifd0
    };
    reader.ifd(at, kind, 0, w, c)
}
