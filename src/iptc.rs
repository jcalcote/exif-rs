// Selected metadata-extractor 2.19.0 IptcReader/IptcDescriptor adaptation.
// Copyright 2002-2019 Drew Noakes and contributors, Apache-2.0. Modified to use
// borrowed records and bounded rescans rather than owned arrays of tag values.
use crate::input;
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix};
use std::fmt::Write;

#[derive(Clone, Copy)]
struct Record {
    tag: u16,
    start: usize,
    len: usize,
    end: usize,
}
fn record(p: Prefix<'_>, at: usize, c: &mut Control<'_>) -> Result<Option<Record>> {
    c.step()?;
    if p.byte(at, c)? != 0x1c {
        if at + 1 == p.len() {
            return Ok(None);
        }
        return Err(Failure::Malformed);
    }
    let tag = (u16::from(p.byte(at + 1, c)?) << 8) | u16::from(p.byte(at + 2, c)?);
    let small = p.u16(at + 3, false, c)?;
    let (start, len) = if small & 0x8000 != 0 {
        (
            at + 7,
            (usize::from(small & 0x7fff) << 16) | usize::from(p.u16(at + 5, false, c)?),
        )
    } else {
        (at + 5, usize::from(small))
    };
    p.slice(start, len)?;
    Ok(Some(Record {
        tag,
        start,
        len,
        end: start + len,
    }))
}
fn name(tag: u16) -> Option<&'static str> {
    Some(match tag {
        0x015a => "Coded Character Set",
        0x0200 => "Application Record Version",
        0x0205 => "Object Name",
        0x020a => "Urgency",
        0x0219 => "Keywords",
        0x0237 => "Date Created",
        0x023c => "Time Created",
        0x0250 => "By-line",
        0x0255 => "By-line Title",
        0x025a => "City",
        0x025f => "Province/State",
        0x0265 => "Country/Primary Location Name",
        0x0269 => "Headline",
        0x026e => "Credit",
        0x0273 => "Source",
        0x0274 => "Copyright Notice",
        0x0278 => "Caption/Abstract",
        0x027a => "Caption Writer/Editor",
        _ => return None,
    })
}
#[derive(Clone, Copy)]
enum Charset {
    Guess,
    Utf8,
    Unsupported,
}
fn charset(p: Prefix<'_>, r: Record, c: &mut Control<'_>) -> Result<Charset> {
    Ok(
        if r.len == 3 && p.array::<3>(r.start, c)? == [0x1b, b'%', b'G'] {
            Charset::Utf8
        } else {
            Charset::Unsupported
        },
    )
}
fn string(p: Prefix<'_>, r: Record, charset: Charset, c: &mut Control<'_>) -> Result<Option<Text>> {
    if matches!(charset, Charset::Unsupported) {
        c.partial = true;
        return Ok(None);
    }
    if r.len > crate::MAX_TEXT_BYTES {
        return Err(crate::Failure::Limit(crate::Diagnostic::TextLimit));
    }
    let latin1 = if matches!(charset, Charset::Guess) {
        let mut bytes = [0; crate::MAX_TEXT_BYTES];
        p.read(r.start, &mut bytes[..r.len], c)?;
        std::str::from_utf8(&bytes[..r.len]).is_err()
    } else {
        false
    };
    input::text(p, r.start, r.len, latin1, false, c).map(Some)
}
fn previous(p: Prefix<'_>, at: usize, tag: u16, c: &mut Control<'_>) -> Result<bool> {
    let mut cursor = 0;
    while cursor < at {
        let Some(r) = record(p, cursor, c)? else {
            break;
        };
        if r.tag == tag {
            return Ok(true);
        }
        cursor = r.end;
    }
    Ok(false)
}
fn description(p: Prefix<'_>, tag: u16, c: &mut Control<'_>) -> Result<Option<Text>> {
    let mut cursor = 0;
    let mut count = 0usize;
    let mut mode = Charset::Guess;
    let mut text = Text::new();
    let mut supported = true;
    while cursor < p.len() {
        let Some(r) = record(p, cursor, c)? else {
            break;
        };
        cursor = r.end;
        if r.tag == 0x015a {
            mode = charset(p, r, c)?;
        }
        if r.tag != tag {
            continue;
        }
        count += 1;
        match tag {
            0x015a => {
                text = Text::new();
                if matches!(mode, Charset::Utf8) {
                    let _ = text.write_str("UTF-8");
                } else {
                    c.partial = true;
                    text = input::text(p, r.start, r.len, false, false, c)?;
                }
            }
            0x0200 if r.len >= 2 => {
                text = Text::new();
                let _ = write!(text, "{}", p.u16(r.start, false, c)?);
            }
            0x020a if r.len >= 1 => {
                text = Text::new();
                let _ = write!(text, "{}", p.byte(r.start, c)?);
            }
            _ => {
                let Some(value) = string(p, r, mode, c)? else {
                    supported = false;
                    continue;
                };
                if count > 1 {
                    let _ = text.write_str(if tag == 0x0219 { ";" } else { " " });
                }
                let _ = text.write_str(value.as_str());
                if value.is_overflow() {
                    text.mark_overflow();
                }
            }
        }
    }
    if !supported {
        return Ok(None);
    }
    // The generic Java descriptor summarizes arrays after sixteen values;
    // keyword/date/time overrides still render their entire joined value.
    if count > 16 && !matches!(tag, 0x015a | 0x0200 | 0x020a | 0x0219 | 0x0237 | 0x023c) {
        text = Text::new();
        let _ = write!(text, "[{count} values]");
    }
    if !text.is_overflow() {
        let value = text.as_str();
        if tag == 0x0237 && value.len() == 8 && value.is_ascii() {
            let mut date = Text::new();
            let _ = write!(date, "{}:{}:{}", &value[..4], &value[4..6], &value[6..]);
            text = date;
        } else if tag == 0x023c && matches!(value.len(), 6 | 11) && value.is_ascii() {
            let mut time = Text::new();
            let _ = write!(time, "{}:{}:{}", &value[..2], &value[2..4], &value[4..]);
            text = time;
        }
    }
    Ok(Some(text))
}
pub(crate) fn extract(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let mut cursor = 0;
    while cursor < p.len() {
        let at = cursor;
        let Some(r) = record(p, cursor, c)? else {
            break;
        };
        cursor = r.end;
        let Some(name) = name(r.tag) else {
            c.partial = true;
            continue;
        };
        if !previous(p, at, r.tag, c)? {
            if let Some(value) = description(p, r.tag, c)? {
                w.text("IPTC", name, &value, c)?;
            }
        }
    }
    Ok(())
}
