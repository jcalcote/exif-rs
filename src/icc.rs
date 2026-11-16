// Selected metadata-extractor 2.19.0 IccReader/IccDescriptor adaptation.
// Copyright 2002-2019 Drew Noakes and contributors, Apache-2.0. Modified to use
// bounded virtual source spans and caller scratch instead of profile byte arrays.
use crate::input;
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix, SCRATCH_BYTES};
use std::fmt::Write;

const FRAGMENTS: usize = 255;
const TABLE_BYTES: usize = FRAGMENTS * 16;
const _: () = assert!(TABLE_BYTES <= SCRATCH_BYTES);
#[derive(Clone, Copy)]
struct Profile<'source, 'table> {
    source: Prefix<'source>,
    table: Option<&'table [u8]>,
    count: usize,
    len: usize,
}
impl Profile<'_, '_> {
    fn read(self, offset: usize, out: &mut [u8], c: &mut Control<'_>) -> Result<()> {
        if offset > self.len || out.len() > self.len - offset {
            return Err(Failure::Malformed);
        }
        let Some(table) = self.table else {
            return self.source.read(offset, out, c);
        };
        let mut skip = offset;
        let mut done = 0;
        for entry in 0..self.count {
            c.step()?;
            let (start, len) = fragment(table, entry)?;
            if skip >= len {
                skip -= len;
                continue;
            }
            let take = (len - skip).min(out.len() - done);
            self.source
                .read(start + skip, &mut out[done..done + take], c)?;
            done += take;
            skip = 0;
            if done == out.len() {
                return Ok(());
            }
        }
        if out.is_empty() {
            Ok(())
        } else {
            Err(Failure::Malformed)
        }
    }
    fn array<const N: usize>(self, offset: usize, c: &mut Control<'_>) -> Result<[u8; N]> {
        let mut bytes = [0; N];
        self.read(offset, &mut bytes, c)?;
        Ok(bytes)
    }
    fn u32(self, offset: usize, c: &mut Control<'_>) -> Result<u32> {
        Ok(u32::from_be_bytes(self.array(offset, c)?))
    }
    fn u16(self, offset: usize, c: &mut Control<'_>) -> Result<u16> {
        Ok(u16::from_be_bytes(self.array(offset, c)?))
    }
    fn string(self, offset: usize, len: usize, ascii: bool, c: &mut Control<'_>) -> Result<Text> {
        if offset > self.len || len > self.len - offset {
            return Err(Failure::Malformed);
        }
        if len > crate::MAX_TEXT_BYTES {
            return Err(crate::Failure::Limit(crate::Diagnostic::TextLimit));
        }
        let mut bytes = [0; crate::MAX_TEXT_BYTES];
        self.read(offset, &mut bytes[..len], c)?;
        if ascii {
            let mut text = Text::new();
            for byte in &bytes[..len] {
                c.step()?;
                text.push(if byte.is_ascii() {
                    char::from(*byte)
                } else {
                    '\u{fffd}'
                });
            }
            Ok(text)
        } else {
            let parts = [&bytes[..len]];
            let source = Prefix::new(&parts, len).map_err(|_| Failure::Malformed)?;
            input::text(source, 0, len, false, false, c)
        }
    }
}
pub(crate) fn extract(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    profile(
        Profile {
            source: p,
            table: None,
            count: 0,
            len: p.len(),
        },
        w,
        c,
    )
}
pub(crate) fn is_fragment(p: Prefix<'_>, c: &mut Control<'_>) -> Result<bool> {
    Ok(p.len() >= 11 && p.array::<11>(0, c)?.eq_ignore_ascii_case(b"ICC_PROFILE"))
}
pub(crate) fn jpeg(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let table = &mut scratch[..TABLE_BYTES];
    c.work(TABLE_BYTES)?;
    table.fill(0);
    let mut at = 2;
    let mut expected = None;
    let mut found = 0usize;
    let mut total = 0usize;
    while at < p.len() {
        c.step()?;
        if p.byte(at, c)? != 0xff {
            return Err(Failure::Malformed);
        }
        while p.byte(at, c)? == 0xff {
            c.step()?;
            at += 1;
        }
        let marker = p.byte(at, c)?;
        at += 1;
        if marker == 0xda || marker == 0xd9 {
            break;
        }
        if matches!(marker, 0x01 | 0xd0..=0xd8) {
            continue;
        }
        let len = usize::from(p.u16(at, false, c)?);
        if len < 2 {
            return Err(Failure::Malformed);
        }
        let start = at + 2;
        let body = p.slice(start, len - 2)?;
        at += len;
        if marker != 0xe2 || !is_fragment(body, c)? {
            continue;
        }
        if body.len() < 14 || body.byte(11, c)? != 0 {
            return Err(Failure::Malformed);
        }
        let sequence = usize::from(body.byte(12, c)?);
        let count = usize::from(body.byte(13, c)?);
        if sequence == 0 || count == 0 || sequence > count {
            return Err(Failure::Malformed);
        }
        if expected.is_some_and(|previous| previous != count) {
            return Err(Failure::Malformed);
        }
        expected = Some(count);
        let slot = (sequence - 1) * 16;
        if table[slot..slot + 8] != [0; 8] {
            return Err(Failure::Malformed);
        }
        let payload_start = start + 14;
        let payload_len = body.len() - 14;
        set_fragment(table, sequence - 1, payload_start, payload_len)?;
        total = total.checked_add(payload_len).ok_or(Failure::Malformed)?;
        found += 1;
    }
    let count = expected.ok_or(Failure::Malformed)?;
    if found != count {
        return Err(Failure::Malformed);
    }
    for slot in 0..count {
        c.step()?;
        if table[slot * 16..slot * 16 + 8] == [0; 8] {
            return Err(Failure::Malformed);
        }
    }
    profile(
        Profile {
            source: p,
            table: Some(table),
            count,
            len: total,
        },
        w,
        c,
    )
}
fn decimal(value: f64) -> Text {
    let mut text = Text::new();
    let _ = write!(text, "{value:.3}");
    text.trim_decimal();
    text
}
// Match the Java reader's float[] conversion for signed 15.16 header values.
#[allow(clippy::cast_precision_loss)]
fn fixed(raw: u32) -> f64 {
    f64::from(raw as i32 as f32 / 65536.0)
}
fn four(
    p: Profile<'_, '_>,
    offset: usize,
    name: &str,
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    if p.u32(offset, c)? != 0 {
        w.text("ICC Profile", name, &p.string(offset, 4, false, c)?, c)?;
    }
    Ok(())
}
// Keep ordinary header rendering and profile-offset validation source-auditable.
#[allow(clippy::too_many_lines)]
fn profile(mut p: Profile<'_, '_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let declared = p.u32(0, c)? as usize;
    if declared < 132 || declared > p.len {
        return Err(Failure::Malformed);
    }
    p.len = declared;
    w.number("ICC Profile", "Profile Size", declared, c)?;
    four(p, 4, "CMM Type", w, c)?;
    let version = p.u32(8, c)?;
    if version != 0 {
        w.number(
            "ICC Profile",
            "Version",
            format_args!(
                "{}.{}.{}",
                version >> 24,
                (version >> 20) & 15,
                (version >> 16) & 15
            ),
            c,
        )?;
    }
    if p.u32(12, c)? != 0 {
        let code = p.array::<4>(12, c)?;
        let description = match &code {
            b"scnr" => Some("Input Device"),
            b"mntr" => Some("Display Device"),
            b"prtr" => Some("Output Device"),
            b"link" => Some("DeviceLink"),
            b"spac" => Some("ColorSpace Conversion"),
            b"abst" => Some("Abstract"),
            b"nmcl" => Some("Named Color"),
            _ => None,
        };
        if let Some(description) = description {
            w.put("ICC Profile", "Class", description, c)?;
        } else {
            w.number(
                "ICC Profile",
                "Class",
                format_args!("Unknown ({})", p.string(12, 4, false, c)?.as_str()),
                c,
            )?;
        }
    }
    four(p, 16, "Color space", w, c)?;
    four(p, 20, "Profile Connection Space", w, c)?;
    let year = p.u16(24, c)?;
    let month = p.u16(26, c)?;
    let day = p.u16(28, c)?;
    let hour = p.u16(30, c)?;
    let minute = p.u16(32, c)?;
    let second = p.u16(34, c)?;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    if year > 0 && day > 0 && day <= days && hour < 24 && minute < 60 && second < 60 {
        w.number(
            "ICC Profile",
            "Profile Date/Time",
            format_args!("{year:04}:{month:02}:{day:02} {hour:02}:{minute:02}:{second:02}"),
            c,
        )?;
    } else {
        c.partial = true;
    }
    four(p, 36, "Signature", w, c)?;
    if p.u32(40, c)? != 0 {
        let code = p.array::<4>(40, c)?;
        let description = match &code {
            b"APPL" => Some("Apple Computer, Inc."),
            b"MSFT" => Some("Microsoft Corporation"),
            b"SGI " => Some("Silicon Graphics, Inc."),
            b"SUNW" => Some("Sun Microsystems, Inc."),
            b"TGNT" => Some("Taligent, Inc."),
            _ => None,
        };
        if let Some(description) = description {
            w.put("ICC Profile", "Primary Platform", description, c)?;
        } else {
            w.number(
                "ICC Profile",
                "Primary Platform",
                format_args!("Unknown ({})", p.string(40, 4, false, c)?.as_str()),
                c,
            )?;
        }
    }
    if p.u32(44, c)? != 0 {
        w.number("ICC Profile", "CMM Flags", p.u32(44, c)? as i32, c)?;
    }
    four(p, 48, "Device manufacturer", w, c)?;
    let model = p.u32(52, c)? as i32;
    if model != 0 {
        if model <= 0x2020_2020 {
            w.number("ICC Profile", "Device model", model, c)?;
        } else {
            four(p, 52, "Device model", w, c)?;
        }
    }
    let attributes = i64::from_be_bytes(p.array(56, c)?);
    if attributes != 0 {
        w.number("ICC Profile", "Device attributes", attributes, c)?;
    }
    let intent = p.u32(64, c)? as i32;
    if intent != 0 {
        let description = match intent {
            1 => Some("Media-Relative Colorimetric"),
            2 => Some("Saturation"),
            3 => Some("ICC-Absolute Colorimetric"),
            _ => None,
        };
        if let Some(description) = description {
            w.put("ICC Profile", "Rendering Intent", description, c)?;
        } else {
            w.number(
                "ICC Profile",
                "Rendering Intent",
                format_args!("Unknown ({intent})"),
                c,
            )?;
        }
    }
    w.number(
        "ICC Profile",
        "XYZ values",
        format_args!(
            "{} {} {}",
            decimal(fixed(p.u32(68, c)?)).as_str(),
            decimal(fixed(p.u32(72, c)?)).as_str(),
            decimal(fixed(p.u32(76, c)?)).as_str()
        ),
        c,
    )?;
    let count = p.u32(128, c)? as usize;
    let end = count
        .checked_mul(12)
        .and_then(|n| n.checked_add(132))
        .ok_or(Failure::Malformed)?;
    if end > p.len {
        return Err(Failure::Malformed);
    }
    w.number("ICC Profile", "Tag Count", count, c)?;
    for index in 0..count {
        c.step()?;
        let at = 132 + index * 12;
        let tag = p.array::<4>(at, c)?;
        let start = p.u32(at + 4, c)? as usize;
        let len = p.u32(at + 8, c)? as usize;
        if start > p.len || len > p.len - start {
            return Err(Failure::Malformed);
        }
        let name = match &tag {
            b"desc" => "Profile Description",
            b"cprt" => "Profile Copyright",
            b"tech" => "Technology",
            _ => {
                c.partial = true;
                continue;
            }
        };
        // Java stores the last raw payload for each tag before descriptions are
        // rendered. Do not retain an earlier value if a later duplicate needs an
        // unsupported descriptor. Only three selected tag IDs incur this rescan.
        let mut replaced = false;
        for later in index + 1..count {
            c.step()?;
            if p.array::<4>(132 + later * 12, c)? == tag {
                replaced = true;
                break;
            }
        }
        if replaced {
            continue;
        }
        if let Some(value) = tag_description(p, start, len, c)? {
            w.text("ICC Profile", name, &value, c)?;
        }
    }
    Ok(())
}
fn tag_description(
    p: Profile<'_, '_>,
    start: usize,
    len: usize,
    c: &mut Control<'_>,
) -> Result<Option<Text>> {
    if len < 8 {
        return Err(Failure::Malformed);
    }
    let typ = p.array::<4>(start, c)?;
    match &typ {
        b"text" if len >= 9 => p.string(start + 8, len - 9, true, c).map(Some),
        b"desc" if len >= 12 => {
            let size = p.u32(start + 8, c)? as usize;
            if size == 0 || size > len - 12 {
                return Err(Failure::Malformed);
            }
            p.string(start + 12, size - 1, false, c).map(Some)
        }
        b"sig " if len >= 12 => p.string(start + 8, 4, false, c).map(Some),
        _ => {
            c.partial = true;
            Ok(None)
        }
    }
}

fn fragment(table: &[u8], slot: usize) -> Result<(usize, usize)> {
    let at = slot.checked_mul(16).ok_or(Failure::Malformed)?;
    let bytes = table
        .get(at..at.checked_add(16).ok_or(Failure::Malformed)?)
        .ok_or(Failure::Malformed)?;
    let start = usize::try_from(u64::from_le_bytes(
        bytes[..8].try_into().map_err(|_| Failure::Malformed)?,
    ))
    .map_err(|_| Failure::Malformed)?;
    let len = usize::try_from(u64::from_le_bytes(
        bytes[8..].try_into().map_err(|_| Failure::Malformed)?,
    ))
    .map_err(|_| Failure::Malformed)?;
    Ok((start, len))
}
fn set_fragment(table: &mut [u8], slot: usize, start: usize, len: usize) -> Result<()> {
    let at = slot.checked_mul(16).ok_or(Failure::Malformed)?;
    let bytes = table
        .get_mut(at..at.checked_add(16).ok_or(Failure::Malformed)?)
        .ok_or(Failure::Malformed)?;
    bytes[..8].copy_from_slice(
        &u64::try_from(start)
            .map_err(|_| Failure::Malformed)?
            .to_le_bytes(),
    );
    bytes[8..].copy_from_slice(
        &u64::try_from(len)
            .map_err(|_| Failure::Malformed)?
            .to_le_bytes(),
    );
    Ok(())
}
#[cfg(all(test, target_pointer_width = "64"))]
mod offset_tests {
    #[test]
    fn fragment_codec_preserves_offsets_above_u32_without_large_media() {
        let mut table = [0; 16];
        let start = u32::MAX as usize + 17;
        super::set_fragment(&mut table, 0, start, 123).unwrap();
        assert_eq!(super::fragment(&table, 0).unwrap(), (start, 123));
    }
}
