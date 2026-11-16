// Selected metadata-extractor 2.19.0 MOV/MP4/HEIF box adaptation, Apache-2.0.
// Copyright 2002-2019 Drew Noakes and contributors. Modified for borrowed,
// checked box spans, finite work and explicit partial coverage; no sample decode.
use crate::input;
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix};
use std::fmt::Write;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum Kind {
    QuickTime,
    Mp4,
    Heif,
}
impl Kind {
    fn directory(self) -> &'static str {
        match self {
            Self::QuickTime => "QuickTime",
            Self::Mp4 => "MP4",
            Self::Heif => "HEIF",
        }
    }
}
pub(crate) fn extract(
    p: Prefix<'_>,
    kind: Kind,
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let mut picture = false;
    walk(p, kind, 0, &mut picture, w, c)
}
fn walk(
    p: Prefix<'_>,
    kind: Kind,
    depth: usize,
    picture: &mut bool,
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    c.depth(depth)?;
    let mut at = 0;
    while at < p.len() {
        c.step()?;
        let small_size = p.u32(at, false, c)?;
        let typ = p.array::<4>(at + 4, c)?;
        let (size, header) = match small_size {
            0 => (p.len() - at, 8),
            1 => {
                let size = u64::from_be_bytes(p.array(at + 8, c)?);
                (usize::try_from(size).map_err(|_| Failure::Malformed)?, 16)
            }
            n => (n as usize, 8),
        };
        if size < header {
            return Err(Failure::Malformed);
        }
        let end = at.checked_add(size).ok_or(Failure::Malformed)?;
        if end > p.len() {
            // The immutable prefix can end in a large media payload. Preserve
            // preceding metadata with explicit incomplete coverage, never read on.
            if typ == *b"mdat" {
                c.partial = true;
                return Ok(());
            }
            return Err(Failure::Malformed);
        }
        let body = p.slice(at + header, size - header)?;
        match &typ {
            b"ftyp" => file_type(body, kind, w, c)?,
            b"moov" => walk(body, kind, depth + 1, picture, w, c)?,
            b"meta" if kind == Kind::Heif => walk(
                body.slice(4, body.len().checked_sub(4).ok_or(Failure::Malformed)?)?,
                kind,
                depth + 1,
                picture,
                w,
                c,
            )?,
            b"iprp" | b"ipco" if kind == Kind::Heif => walk(body, kind, depth + 1, picture, w, c)?,
            b"mvhd" if kind != Kind::Heif => movie_header(body, kind, w, c)?,
            b"hdlr" if kind == Kind::Heif => {
                *picture = body.array::<4>(8, c)? == *b"pict";
                if !*picture {
                    c.partial = true;
                }
            }
            b"ispe" if kind == Kind::Heif && *picture => {
                let width = body.u32(4, false, c)?;
                let height = body.u32(8, false, c)?;
                if !w.has("HEIF", "Width", c)? && !w.has("HEIF", "Height", c)? {
                    w.number("HEIF", "Width", format_args!("{width} pixels"), c)?;
                    w.number("HEIF", "Height", format_args!("{height} pixels"), c)?;
                }
            }
            b"irot" if kind == Kind::Heif && *picture => {
                let rotation = body.byte(0, c)? & 3;
                if !w.has("HEIF", "Rotation", c)? {
                    w.number(
                        "HEIF",
                        "Rotation",
                        format_args!("{} degrees", u16::from(rotation) * 90),
                        c,
                    )?;
                }
            }
            b"pixi" if kind == Kind::Heif && *picture => {
                let count = usize::from(body.byte(4, c)?);
                body.slice(5, count)?;
                let mut bits = Text::new();
                if count > 16 {
                    let _ = write!(bits, "[{count} values]");
                } else {
                    for i in 0..count {
                        c.step()?;
                        if i > 0 {
                            let _ = bits.write_str(" ");
                        }
                        let _ = write!(bits, "{}", body.byte(5 + i, c)?);
                    }
                }
                if !w.has("HEIF", "Bits Per Channel", c)? {
                    w.text("HEIF", "Bits Per Channel", &bits, c)?;
                }
            }
            // Format-defined padding/media bytes carry no selected metadata.
            b"mdat" | b"free" | b"skip" | b"wide" => {}
            _ => c.partial = true,
        }
        at = end;
    }
    Ok(())
}
fn brand(value: &str) -> Option<&'static str> {
    // Fixture-driven subset of the pinned dictionaries; other descriptions are
    // explicit coverage gaps rather than invented substitutions.
    match value {
        "isom" => Some("MP4\u{a0} Base Media v1 [IS0 14496-12:2003]"),
        "mp42" => Some("MP4 v2 [ISO 14496-14]"),
        "qt  " => Some("Apple QuickTime (.MOV/QT)"),
        _ => None,
    }
}
fn file_type(p: Prefix<'_>, kind: Kind, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    if p.len() < 8 || (p.len() - 8) % 4 != 0 {
        return Err(Failure::Malformed);
    }
    let directory = kind.directory();
    let major = input::text(p, 0, 4, true, false, c)?;
    if kind == Kind::Heif {
        w.text(directory, "Major Brand", &major, c)?;
    } else if let Some(description) = brand(major.as_str()) {
        w.put(directory, "Major Brand", description, c)?;
    } else {
        c.partial = true;
    }
    w.number(directory, "Minor Version", p.u32(4, false, c)?, c)?;
    let count = (p.len() - 8) / 4;
    let mut compatible = Text::new();
    let mut supported = true;
    if kind == Kind::Heif && count > 16 {
        let _ = write!(compatible, "[{count} values]");
    } else {
        if kind != Kind::Heif {
            let _ = compatible.write_str("[");
        }
        for i in 0..count {
            c.step()?;
            if compatible.is_overflow() {
                break;
            }
            if i > 0 {
                let _ = compatible.write_str(if kind == Kind::Heif { " " } else { ", " });
            }
            let value = input::text(p, 8 + 4 * i, 4, true, false, c)?;
            if kind == Kind::Heif {
                let _ = compatible.write_str(value.as_str());
            } else if let Some(description) = brand(value.as_str()) {
                let _ = compatible.write_str(description);
            } else {
                supported = false;
                c.partial = true;
            }
        }
        if kind != Kind::Heif {
            let _ = compatible.write_str("]");
        }
    }
    if supported {
        w.text(directory, "Compatible Brands", &compatible, c)?;
    }
    Ok(())
}
fn movie_header(p: Prefix<'_>, kind: Kind, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let version = p.byte(0, c)?;
    let (timescale, duration, after_duration) = match version {
        0 => (p.u32(12, false, c)?, u64::from(p.u32(16, false, c)?), 20),
        1 if kind == Kind::Mp4 => (
            p.u32(20, false, c)?,
            u64::from_be_bytes(p.array(24, c)?),
            32,
        ),
        _ => {
            c.partial = true;
            return Ok(());
        }
    };
    p.slice(0, after_duration + 80)?;
    if timescale == 0 {
        return Err(Failure::Malformed);
    }
    let directory = kind.directory();
    w.number(directory, "Media Time Scale", timescale, c)?;
    w.number(directory, "Duration", duration, c)?;
    let seconds = duration.div_ceil(u64::from(timescale));
    w.number(
        directory,
        "Duration in Seconds",
        format_args!(
            "{:02}:{:02}:{:02}",
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60
        ),
        c,
    )?;
    let rate = p.u32(after_duration, false, c)? as i32;
    let volume = p.u16(after_duration + 4, false, c)? as i16;
    // Preserve the pinned reader's existing presentation formula. Common
    // identity rate/volume fixtures do not require decoder/runtime state.
    let preferred_rate = f64::from(rate >> 16) + f64::from(rate & 0xffff) / 16.0;
    let preferred_volume = f64::from(volume >> 8)
        + f64::from(volume & 0xff) / if kind == Kind::QuickTime { 8.0 } else { 4.0 };
    w.number(directory, "Preferred Rate", preferred_rate, c)?;
    w.number(directory, "Preferred Volume", preferred_volume, c)?;
    if kind == Kind::Mp4 {
        let mut matrix = Text::new();
        for i in 0..9 {
            c.step()?;
            if i > 0 {
                let _ = matrix.write_str(" ");
            }
            let _ = write!(
                matrix,
                "{}",
                p.u32(after_duration + 16 + 4 * i, false, c)? as i32
            );
        }
        w.text(directory, "Transformation Matrix", &matrix, c)?;
    } else {
        for (i, name) in [
            "Preview Time",
            "Preview Duration",
            "Poster Time",
            "Selection Time",
            "Selection Duration",
            "Current Time",
        ]
        .iter()
        .enumerate()
        {
            w.number(
                directory,
                name,
                p.u32(after_duration + 52 + 4 * i, false, c)?,
                c,
            )?;
        }
    }
    w.number(
        directory,
        "Next Track ID",
        p.u32(after_duration + 76, false, c)?,
        c,
    )?;
    // Creation/modification Date descriptions and track/sample handlers remain
    // explicit next-checkpoint fields. Never label this full reader coverage.
    c.partial = true;
    Ok(())
}
