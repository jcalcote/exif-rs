// Adapted from metadata-extractor 2.19.0 RiffReader, WavRiffHandler,
// AviRiffHandler, WebpRiffHandler and Mp3Reader. Apache-2.0; see resources.
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix};
use crate::{input, tables, tiff};
use std::fmt::Write;

// Conversion is intentionally truncating for clock components after rejecting
// negative/nonfinite input; no float-derived value is used for memory indexing.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
fn duration(seconds: f64) -> Text {
    let mut text = Text::new();
    if !seconds.is_finite() || seconds < 0.0 {
        return text;
    }
    let whole = seconds as u64;
    // Retain the legacy formatting, including rounded seconds at minute edges.
    let _ = write!(
        text,
        "{:02}:{:02}:{:02}",
        whole / 3600,
        (whole / 60) % 60,
        (seconds - (whole / 60 * 60) as f64).round() as u64
    );
    text
}

pub(crate) fn wav(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let mut bytes_per_second = None;
    walk(
        p,
        12,
        p.len(),
        0,
        b"WAVE",
        c,
        &mut |id, body, declared, list, c| {
            match id {
                b"fmt " => {
                    let format = body.u16(0, true, c)?;
                    w.put("WAV", "Format", tables::wav_encoding(format), c)?;
                    for (name, at) in [("Channels", 2), ("Block Alignment", 12)] {
                        w.number("WAV", name, body.u16(at, true, c)?, c)?;
                    }
                    let rate = body.u32(8, true, c)?;
                    bytes_per_second = Some(rate);
                    w.number("WAV", "Samples Per Second", body.u32(4, true, c)?, c)?;
                    w.number("WAV", "Bytes Per Second", rate, c)?;
                    if format == 1 {
                        w.number("WAV", "Bits Per Sample", body.u16(14, true, c)?, c)?;
                    }
                }
                b"data" => {
                    if let Some(rate) = bytes_per_second.filter(|n| *n != 0) {
                        w.text(
                            "WAV",
                            "Duration",
                            &duration(
                                f64::from(u32::try_from(declared).map_err(|_| Failure::Malformed)?)
                                    / f64::from(rate),
                            ),
                            c,
                        )?;
                    }
                }
                _ if list == b"INFO" => {
                    let name = match id {
                        b"IART" => "Artist",
                        b"INAM" => "Title",
                        b"IPRD" => "Product",
                        b"ITRK" => "Track Number",
                        b"ICRD" => "Date Created",
                        b"IGNR" => "Genre",
                        b"ICMT" => "Comments",
                        b"ICOP" => "Copyright",
                        b"ISFT" => "Software",
                        _ => {
                            c.partial = true;
                            return Ok(());
                        }
                    };
                    let text = input::text(body, 0, body.len(), false, true, c)?;
                    w.text("WAV", name, &text, c)?;
                }
                _ => c.partial = true,
            }
            Ok(())
        },
    )
}
pub(crate) fn avi(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let mut video = false;
    let mut audio = false;
    walk(p, 12, p.len(), 0, b"AVI ", c, &mut |id, body, _, _, c| {
        match id {
            b"avih" => {
                w.number(
                    "AVI",
                    "Width",
                    format_args!("{} pixels", body.u32(32, true, c)? as i32),
                    c,
                )?;
                w.number(
                    "AVI",
                    "Height",
                    format_args!("{} pixels", body.u32(36, true, c)? as i32),
                    c,
                )?;
                w.number("AVI", "Stream Count", body.u32(24, true, c)? as i32, c)?;
            }
            b"strh" => {
                let typ = body.array::<4>(0, c)?;
                let scale = body.u32(20, true, c)?;
                let rate = body.u32(24, true, c)?;
                if scale == 0 {
                    return Err(Failure::Malformed);
                }
                let ratio = f64::from(rate) / f64::from(scale);
                if typ == *b"vids" && !video {
                    video = true;
                    w.number("AVI", "Frames Per Second", ratio, c)?;
                    if rate != 0 {
                        w.text(
                            "AVI",
                            "Duration",
                            &duration(f64::from(body.u32(32, true, c)?) / ratio),
                            c,
                        )?;
                    }
                    w.text(
                        "AVI",
                        "Video Codec",
                        &input::text(body, 4, 4, false, false, c)?,
                        c,
                    )?;
                } else if typ == *b"auds" && !audio {
                    audio = true;
                    w.number("AVI", "Samples Per Second", ratio, c)?;
                } else {
                    c.partial = true;
                }
            }
            b"IDIT" => {
                let len = if body.len() == 26 && body.array::<2>(24, c)? == [10, 0] {
                    24
                } else {
                    body.len()
                };
                w.text(
                    "AVI",
                    "Date/Time Original",
                    &input::text(body, 0, len, false, false, c)?,
                    c,
                )?;
            }
            // strf, INFO entries and other unimplemented metadata cannot be
            // presented as complete merely because the main header decoded.
            _ => c.partial = true,
        }
        Ok(())
    })
}

pub(crate) fn webp(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    walk(p, 12, p.len(), 0, b"WEBP", c, &mut |id, body, _, _, c| {
        let (width, height, flags) = match id {
            b"VP8X" => {
                let bytes = body.array::<10>(0, c)?;
                (
                    1 + u32::from_le_bytes([bytes[4], bytes[5], bytes[6], 0]),
                    1 + u32::from_le_bytes([bytes[7], bytes[8], bytes[9], 0]),
                    Some((bytes[0] & 16 != 0, bytes[0] & 2 != 0)),
                )
            }
            b"VP8L" => {
                if body.byte(0, c)? != 0x2f {
                    return Err(Failure::Malformed);
                }
                let bits = body.u32(1, true, c)?;
                (
                    (bits & 0x3fff) + 1,
                    ((bits >> 14) & 0x3fff) + 1,
                    Some((bits & (1 << 28) != 0, false)),
                )
            }
            b"VP8 " => {
                if body.array::<3>(3, c)? != [0x9d, 1, 0x2a] {
                    return Err(Failure::Malformed);
                }
                (
                    u32::from(body.u16(6, true, c)? & 0x3fff),
                    u32::from(body.u16(8, true, c)? & 0x3fff),
                    None,
                )
            }
            b"EXIF" => {
                tiff::extract(body, scratch, w, c)?;
                return Ok(());
            }
            b"ICCP" | b"XMP " | b"ANIM" | b"ANMF" => {
                c.partial = true;
                return Ok(());
            }
            // ALPH contains coded alpha samples, not extracted metadata.
            b"ALPH" => return Ok(()),
            _ => {
                c.partial = true;
                return Ok(());
            }
        };
        w.number("WebP", "Image Width", width, c)?;
        w.number("WebP", "Image Height", height, c)?;
        if let Some((alpha, animation)) = flags {
            w.put("WebP", "Has Alpha", if alpha { "true" } else { "false" }, c)?;
            w.put(
                "WebP",
                "Is Animation",
                if animation { "true" } else { "false" },
                c,
            )?;
        }
        Ok(())
    })
}

// Shared borrowed FourCC shape matches the visitor arguments.
#[allow(clippy::trivially_copy_pass_by_ref)]
fn walk(
    p: Prefix<'_>,
    mut at: usize,
    end: usize,
    depth: usize,
    list: &[u8; 4],
    c: &mut Control<'_>,
    visit: &mut impl FnMut(&[u8; 4], Prefix<'_>, usize, &[u8; 4], &mut Control<'_>) -> Result<()>,
) -> Result<()> {
    c.depth(depth)?;
    while at < end {
        c.step()?;
        let id = p.array::<4>(at, c)?;
        let len = p.u32(at + 4, true, c)? as usize;
        let start = at.checked_add(8).ok_or(Failure::Malformed)?;
        let next = start.checked_add(len).ok_or(Failure::Malformed)?;
        let available = end.saturating_sub(start).min(len);
        let body = p.slice(start, available)?;
        if matches!(&id, b"JUNK" | b"PAD ") {
            // These RIFF chunks are explicitly padding, including in LISTs.
        } else if id == *b"LIST" {
            if len < 4 {
                return Err(Failure::Malformed);
            }
            let typ = body.array::<4>(0, c)?;
            if matches!(&typ, b"INFO" | b"hdrl" | b"strl") {
                walk(p, start + 4, next.min(end), depth + 1, &typ, c, visit)?;
            } else if typ != *b"movi" || list != b"AVI " {
                // Only the AVI root's movi list is known sample payload. Other
                // unhandled list types may contain metadata (e.g. WAVE adtl).
                c.partial = true;
            }
        } else {
            visit(&id, body, len, list, c)?;
        }
        if next > end {
            c.partial = true;
            return Ok(());
        }
        at = next.checked_add(len & 1).ok_or(Failure::Malformed)?;
    }
    Ok(())
}

pub(crate) fn mp3(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let header = p.u32(0, false, c)?;
    if header >> 21 != 0x7ff {
        return Err(Failure::Malformed);
    }
    let id = (header >> 19) & 3;
    if id != 2 && id != 3 {
        return Err(Failure::Malformed);
    }
    let layer = ((header >> 17) & 3) as usize;
    let rate_index = ((header >> 12) & 15) as usize;
    let sample_index = ((header >> 10) & 3) as usize;
    let rate = if sample_index < 3 {
        [44100, 48000, 32000][sample_index] / if id == 2 { 2 } else { 1 }
    } else {
        return Err(Failure::Malformed);
    };
    w.put("MP3", "ID", if id == 3 { "MPEG-1" } else { "MPEG-2" }, c)?;
    w.put(
        "MP3",
        "Layer",
        ["Not defined", "Layer III", "Layer II", "Layer I"][layer],
        c,
    )?;
    w.number("MP3", "Frequency", rate, c)?;
    w.put(
        "MP3",
        "Mode",
        ["Stereo", "Joint stereo", "Dual channel", "Mono"][((header >> 6) & 3) as usize],
        c,
    )?;
    w.put(
        "MP3",
        "Copyright",
        if header & 8 != 0 { "True" } else { "False" },
        c,
    )?;
    match header & 3 {
        0 => w.put("MP3", "Emphasis Method", "none", c)?,
        1 => w.put("MP3", "Emphasis Method", "50/15ms", c)?,
        3 => w.put("MP3", "Emphasis Method", "CCITT j.17", c)?,
        _ => {}
    }
    if rate_index != 0 && rate_index != 15 {
        // Literal bitrate table from the pinned Java reader, including its legacy values.
        const BITRATES: [[u16; 6]; 14] = [
            [32, 32, 32, 32, 32, 8],
            [64, 48, 40, 64, 48, 16],
            [96, 56, 48, 96, 56, 24],
            [128, 64, 56, 128, 64, 32],
            [160, 80, 64, 160, 80, 64],
            [192, 96, 80, 192, 96, 80],
            [224, 112, 96, 224, 112, 56],
            [256, 128, 112, 256, 128, 64],
            [288, 160, 128, 28, 160, 128],
            [320, 192, 160, 320, 192, 160],
            [352, 224, 192, 352, 224, 112],
            [384, 256, 256, 384, 256, 128],
            [416, 320, 256, 416, 320, 256],
            [448, 384, 320, 448, 384, 320],
        ];
        let column = if id == 2 {
            match layer {
                1 => 5,
                2 => 4,
                3 => 3,
                _ => 0,
            }
        } else {
            match layer {
                1 => 2,
                2 => 1,
                _ => 0,
            }
        };
        let bitrate = u32::from(BITRATES[rate_index - 1][column]);
        w.number("MP3", "Bitrate", bitrate, c)?;
        w.number(
            "MP3",
            "Frame Size",
            format_args!("{} bytes", (bitrate * 1000 * 144) / rate),
            c,
        )?;
    }
    Ok(())
}
