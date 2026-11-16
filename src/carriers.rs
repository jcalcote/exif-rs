// Selected metadata-extractor 2.19.0 RAF/PSD/EPS source adaptation.
// Copyright 2002-2019 Drew Noakes and contributors, Apache-2.0. Changes include
// borrowed segmented spans, caller-funded storage and explicit coverage outcomes.
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix};
use crate::{icc, images, input, iptc, tiff};
use std::fmt::Write;

pub(crate) fn raf(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    // Pinned RAF reader searches only this initial window, not its whole payload.
    for at in 0..p.len().min(512).saturating_sub(2) {
        c.step()?;
        if p.array::<3>(at, c)? == [0xff, 0xd8, 0xff] {
            return images::jpeg(p.slice(at, p.len() - at)?, scratch, w, c);
        }
    }
    Err(Failure::Malformed)
}

pub(crate) fn psd(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let version = p.u16(4, false, c)?;
    if version != 1 && version != 2 {
        return Err(Failure::Malformed);
    }
    let channels = p.u16(12, false, c)?;
    w.number(
        "PSD Header",
        "Channel Count",
        format_args!("{channels} channel{}", if channels == 1 { "" } else { "s" }),
        c,
    )?;
    for (name, at) in [("Image Height", 14), ("Image Width", 18)] {
        let n = p.u32(at, false, c)? as i32;
        w.number(
            "PSD Header",
            name,
            format_args!("{n} pixel{}", if n == 1 { "" } else { "s" }),
            c,
        )?;
    }
    let bits = p.u16(22, false, c)?;
    w.number(
        "PSD Header",
        "Bits Per Channel",
        format_args!("{bits} bit{} per channel", if bits == 1 { "" } else { "s" }),
        c,
    )?;
    let color = p.u16(24, false, c)?;
    let description = match color {
        0 => "Bitmap",
        1 => "Grayscale",
        2 => "Indexed",
        3 => "RGB",
        4 => "CMYK",
        7 => "Multichannel",
        8 => "Duotone",
        9 => "Lab",
        _ => "Unknown",
    };
    w.put("PSD Header", "Color Mode", description, c)?;
    let color_len = p.u32(26, false, c)? as usize;
    let resource_at = 30usize.checked_add(color_len).ok_or(Failure::Malformed)?;
    let resource_len = p.u32(resource_at, false, c)? as usize;
    let resources = p.slice(resource_at + 4, resource_len)?;
    photoshop_resources(resources, scratch, w, c)
}

pub(crate) fn photoshop_resources(
    resources: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let mut at = 0;
    while at < resources.len() {
        c.step()?;
        if resources.array::<4>(at, c)? != *b"8BIM" {
            return Err(Failure::Malformed);
        }
        let id = resources.u16(at + 4, false, c)?;
        let name_len = usize::from(resources.byte(at + 6, c)?) + 1;
        let size_at = at
            .checked_add(6 + name_len + (name_len & 1))
            .ok_or(Failure::Malformed)?;
        let size = resources.u32(size_at, false, c)? as usize;
        let data = resources.slice(size_at + 4, size)?;
        match id {
            0x0422 | 0x0423 => tiff::extract(data, scratch, w, c)?,
            0x0404 => iptc::extract(data, w, c)?,
            0x040f => icc::extract(data, w, c)?,
            _ => c.partial = true,
        }
        at = size_at
            .checked_add(4)
            .and_then(|n| n.checked_add(size))
            .and_then(|n| n.checked_add(size & 1))
            .ok_or(Failure::Malformed)?;
        if at > resources.len() {
            return Err(Failure::Malformed);
        }
    }
    Ok(())
}

pub(crate) fn eps(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let text = if p.starts(b"\xc5\xd0\xd3\xc6", c)? {
        let offset = p.u32(4, true, c)? as usize;
        let len = p.u32(8, true, c)? as usize;
        let tif_len = p.u32(24, true, c)? as usize;
        if tif_len != 0 {
            let tif_at = p.u32(20, true, c)? as usize;
            w.number(
                "EPS",
                "TIFF Preview Size",
                format_args!("{tif_len} bytes"),
                c,
            )?;
            w.number(
                "EPS",
                "TIFF Preview Offset",
                format_args!("{tif_at} bytes"),
                c,
            )?;
            tiff::extract(p.slice(tif_at, tif_len)?, scratch, w, c)?;
        } else {
            let wmf_len = p.u32(16, true, c)?;
            if wmf_len != 0 {
                w.number("EPS", "WMF Preview Size", wmf_len, c)?;
                w.number("EPS", "WMF Preview Offset", p.u32(12, true, c)?, c)?;
            }
        }
        p.slice(offset, len)?
    } else {
        p
    };
    eps_comments(text, w, c)
}

fn comment_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "%%Author" => "Author",
        "%%BoundingBox" => "Bounding Box",
        "%%Copyright" => "Copyright",
        "%%CreationDate" => "Creation Date",
        "%%Creator" => "Creator",
        "%%For" => "For",
        "%%Keywords" => "Keywords",
        "%%ModDate" => "Modify Date",
        "%%Pages" => "Pages",
        "%%Routing" => "Routing",
        "%%Subject" => "Subject",
        "%%Title" => "Title",
        "%%Version" => "Version",
        "%%DocumentData" => "Document Data",
        "%%Emulation" => "Emulation",
        "%%Extensions" => "Extensions",
        "%%LanguageLevel" => "Language Level",
        "%%Orientation" => "Orientation",
        "%%PageOrder" => "Page Order",
        "%%OperatorIntervention" => "Operator Intervention",
        "%%OperatorMessage" => "Operator Message",
        "%%ProofMode" => "Proof Mode",
        "%%Requirements" => "Requirements",
        "%%VMlocation" => "VM Location",
        "%%VMusage" => "VM Usage",
        _ => return None,
    })
}
fn trim(
    p: Prefix<'_>,
    mut start: usize,
    mut end: usize,
    c: &mut Control<'_>,
) -> Result<(usize, usize)> {
    while start < end && p.byte(start, c)? <= 32 {
        c.step()?;
        start += 1;
    }
    while start < end && p.byte(end - 1, c)? <= 32 {
        c.step()?;
        end -= 1;
    }
    Ok((start, end - start))
}
fn flush(
    pending: &mut Option<(&'static str, Text)>,
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    if let Some((name, value)) = pending.take() {
        if !w.has("EPS", name, c)? {
            w.text("EPS", name, &value, c)?;
        }
    }
    Ok(())
}
fn eps_comments(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let mut at = 0;
    let mut pending: Option<(&'static str, Text)> = None;
    let mut metadata = false;
    while at < p.len() {
        c.step()?;
        let start = at;
        while at < p.len() && !matches!(p.byte(at, c)?, b'\r' | b'\n') {
            c.step()?;
            at += 1;
        }
        let end = at;
        if at < p.len() {
            at += 1;
        }
        if start == end {
            continue;
        }
        if p.byte(start, c)? != b'%' {
            break;
        }
        let mut colon = start;
        while colon < end && p.byte(colon, c)? != b':' {
            c.step()?;
            colon += 1;
        }
        let (key_at, key_len) = trim(p, start, colon, c)?;
        let key = input::text(p, key_at, key_len, true, false, c)?;
        if matches!(
            key.as_str(),
            "%BeginPhotoshop" | "%%BeginICCProfile" | "%begin_xml_packet"
        ) {
            c.partial = true;
            break;
        }
        if colon == end {
            continue;
        }
        let (value_at, value_len) = trim(p, colon + 1, end, c)?;
        let value = input::text(p, value_at, value_len, true, false, c)?;
        if key.as_str() == "%%+" {
            if let Some((_, previous)) = &mut pending {
                let _ = write!(previous, " {}", value.as_str());
                if value.is_overflow() {
                    previous.mark_overflow();
                }
            }
            continue;
        }
        flush(&mut pending, w, c)?;
        if key.as_str() == "%ImageData" {
            image_data(&value, w, c)?;
            metadata = true;
        } else if let Some(name) = comment_name(key.as_str()) {
            pending = Some((name, value));
            metadata = true;
        }
    }
    if !metadata {
        c.partial = true;
    }
    flush(&mut pending, w, c)
}
fn image_data(value: &Text, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    if value.is_overflow() {
        return Err(Failure::Limit(crate::Diagnostic::TextLimit));
    }
    w.text("EPS", "Image Data", value, c)?;
    let mut parts = value.as_str().split_whitespace();
    let width = parts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or(Failure::Malformed)?;
    let height = parts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or(Failure::Malformed)?;
    let _bits = parts.next().ok_or(Failure::Malformed)?;
    let color = parts
        .next()
        .and_then(|n| n.parse::<u32>().ok())
        .ok_or(Failure::Malformed)?;
    for (name, n) in [("Image Width", width), ("Image Height", height)] {
        if !w.has("EPS", name, c)? {
            w.number("EPS", name, format_args!("{n} pixels"), c)?;
        }
    }
    if !w.has("EPS", "Color Type", c)? {
        w.put(
            "EPS",
            "Color Type",
            match color {
                1 => "Grayscale",
                2 => "Lab",
                3 => "RGB",
                4 => "CMYK",
                _ => "Unknown",
            },
            c,
        )?;
    }
    if (1..=4).contains(&color) && !w.has("EPS", "Ram Size", c)? {
        let bytes = width
            .checked_mul(height)
            .and_then(|n| n.checked_mul(if color == 1 { 1 } else { 3 }))
            .ok_or(Failure::Malformed)?;
        w.number("EPS", "Ram Size", bytes, c)?;
    }
    Ok(())
}
