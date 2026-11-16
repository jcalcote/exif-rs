// Format/header behavior adapted from metadata-extractor 2.19.0 readers and
// directories. Copyright 2002-2019 Drew Noakes and contributors, Apache-2.0.
use crate::packed::{Text, Writer};
use crate::{Control, Failure, ParseResult as Result, Prefix};
use crate::{bmff, carriers, icc, input, iptc, riff, tiff};
use std::fmt::Write;

// Keep supported format dispatch in one source-auditable selection boundary.
#[allow(clippy::too_many_lines)]
pub(crate) fn extract(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let (name, long, mime, ext) = if p.starts(b"\xff\xd8", c)? {
        jpeg(p, scratch, w, c)?;
        (
            "JPEG",
            "Joint Photographic Experts Group",
            "image/jpeg",
            "jpg",
        )
    } else if p.starts(b"II\x2a\0", c)? || p.starts(b"MM\0\x2a", c)? {
        tiff::extract(p, scratch, w, c)?;
        // A conventional TIFF header/IFD offset is not camera-specific RAW evidence.
        if p.starts(b"II\x2a\0\x10\0\0\0CR", c)? {
            ("CR2", "Canon Camera Raw", "", "cr2")
        } else {
            ("TIFF", "Tagged Image File Format", "image/tiff", "tiff")
        }
    } else if p.starts(b"IIRO", c)? || p.starts(b"IIRS", c)? || p.starts(b"MMOR", c)? {
        tiff::extract(p, scratch, w, c)?;
        ("ORF", "Olympus Camera Raw", "", "orf")
    } else if p.starts(b"II\x55\0", c)? {
        tiff::extract(p, scratch, w, c)?;
        ("RW2", "Panasonic Camera Raw", "", "rw2")
    } else if p.starts(b"\x89PNG\r\n\x1a\n", c)? {
        png(p, w, c)?;
        ("PNG", "Portable Network Graphics", "image/png", "png")
    } else if p.starts(b"BM", c)? {
        bmp(p, w, c)?;
        ("BMP", "Device Independent Bitmap", "image/bmp", "bmp")
    } else if p.starts(b"GIF87a", c)? || p.starts(b"GIF89a", c)? {
        gif(p, w, c)?;
        ("GIF", "Graphics Interchange Format", "image/gif", "gif")
    } else if p.starts(b"\0\0\x01\0", c)? || p.starts(b"\0\0\x02\0", c)? {
        ico(p, w, c)?;
        ("ICO", "Windows Icon", "image/x-icon", "ico")
    } else if p.len() >= 4 && p.byte(0, c)? == 10 && p.byte(2, c)? == 1 {
        pcx(p, w, c)?;
        ("PCX", "PiCture eXchange", "image/x-pcx", "pcx")
    } else if p.starts(b"RIFF", c)? {
        match &p.array::<4>(8, c)? {
            b"WAVE" => {
                riff::wav(p, w, c)?;
                ("WAV", "Waveform Audio File Format", "audio/vnd.wave", "wav")
            }
            b"AVI " => {
                riff::avi(p, w, c)?;
                ("AVI", "Audio Video Interleaved", "video/vnd.avi", "avi")
            }
            b"WEBP" => {
                riff::webp(p, scratch, w, c)?;
                ("WebP", "WebP", "image/webp", "webp")
            }
            _ => return Err(Failure::Unknown),
        }
    } else if p.len() >= 4 && p.byte(0, c)? == 0xff && p.byte(1, c)? & 0xe0 == 0xe0 {
        riff::mp3(p, w, c)?;
        ("MP3", "MPEG Audio Layer III", "audio/mpeg", "mp3")
    } else if p.starts(b"%PDF-", c)? {
        c.partial = true;
        ("PDF", "Portable Document Format", "application/pdf", "pdf")
    } else if p.starts(b"8BPS", c)? {
        carriers::psd(p, scratch, w, c)?;
        (
            "PSD",
            "Photoshop Document",
            "image/vnd.adobe.photoshop",
            "psd",
        )
    } else if p.starts(b"FUJIFILMCCD-RAW", c)? {
        carriers::raf(p, scratch, w, c)?;
        ("RAF", "FujiFilm Camera Raw", "", "raf")
    } else if p.starts(b"%!PS-Adobe-", c)? || p.starts(b"\xc5\xd0\xd3\xc6", c)? {
        carriers::eps(p, scratch, w, c)?;
        (
            "EPS",
            "Encapsulated PostScript",
            "application/postscript",
            "eps",
        )
    } else if p.len() >= 12 && p.slice(4, 4)?.starts(b"ftyp", c)? {
        match &p.array::<4>(8, c)? {
            b"qt  " => {
                bmff::extract(p, bmff::Kind::QuickTime, w, c)?;
                ("MOV", "QuickTime Movie", "video/quicktime", "mov")
            }
            b"heic" | b"heix" | b"mif1" | b"hevc" => {
                bmff::extract(p, bmff::Kind::Heif, w, c)?;
                (
                    "HEIF",
                    "High Efficiency Image File Format",
                    "image/heif",
                    "heif",
                )
            }
            _ => {
                bmff::extract(p, bmff::Kind::Mp4, w, c)?;
                ("MP4", "MPEG-4 Part 14", "video/mp4", "mp4")
            }
        }
    } else {
        return Err(Failure::Unknown);
    };
    w.put("File Type", "Detected File Type Name", name, c)?;
    w.put("File Type", "Detected File Type Long Name", long, c)?;
    if !mime.is_empty() {
        w.put("File Type", "Detected MIME Type", mime, c)?;
    }
    w.put("File Type", "Expected File Name Extension", ext, c)
}

// Keep the single marker dispatch together for source comparison.
#[allow(clippy::too_many_lines)]
pub(crate) fn jpeg(
    p: Prefix<'_>,
    scratch: &mut [u8],
    w: &mut Writer<'_>,
    c: &mut Control<'_>,
) -> Result<()> {
    let mut at = 2;
    let mut icc_done = false;
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
            return Ok(());
        }
        if matches!(marker, 0x01 | 0xd0..=0xd8) {
            continue;
        }
        let len = usize::from(p.u16(at, false, c)?);
        if len < 2 {
            return Err(Failure::Malformed);
        }
        let body = p.slice(at + 2, len - 2)?;
        at += len;
        match marker {
            0xc0..=0xcf if !matches!(marker, 0xc4 | 0xc8 | 0xcc) => {
                let names = [
                    "Baseline",
                    "Extended sequential, Huffman",
                    "Progressive, Huffman",
                    "Lossless, Huffman",
                    "",
                    "Differential sequential, Huffman",
                    "Differential progressive, Huffman",
                    "Differential lossless, Huffman",
                    "Reserved for JPEG extensions",
                    "Extended sequential, arithmetic",
                    "Progressive, arithmetic",
                    "Lossless, arithmetic",
                    "",
                    "Differential sequential, arithmetic",
                    "Differential progressive, arithmetic",
                    "Differential lossless, arithmetic",
                ];
                w.put(
                    "JPEG",
                    "Compression Type",
                    names[usize::from(marker - 0xc0)],
                    c,
                )?;
                w.number(
                    "JPEG",
                    "Data Precision",
                    format_args!("{} bits", body.byte(0, c)?),
                    c,
                )?;
                w.number(
                    "JPEG",
                    "Image Height",
                    format_args!("{} pixels", body.u16(1, false, c)?),
                    c,
                )?;
                w.number(
                    "JPEG",
                    "Image Width",
                    format_args!("{} pixels", body.u16(3, false, c)?),
                    c,
                )?;
                let components = body.byte(5, c)?;
                body.slice(6, usize::from(components) * 3)?;
                w.number("JPEG", "Number of Components", components, c)?;
                // Component descriptors have semantic names and sampling factors.
                for n in 0..usize::from(components.min(4)) {
                    let at = 6 + n * 3;
                    let component = body.byte(at, c)?;
                    let sample = body.byte(at + 1, c)?;
                    let quant = body.byte(at + 2, c)?;
                    let mut name = Text::new();
                    let _ = write!(name, "Component {}", n + 1);
                    let component_name = match component {
                        1 => "Y",
                        2 => "Cb",
                        3 => "Cr",
                        4 => "I",
                        5 => "Q",
                        _ => "Unknown",
                    };
                    w.number("JPEG", name.as_str(), format_args!("{component_name} component: Quantization table {quant}, Sampling factors {} horiz/{} vert", sample >> 4, sample & 15), c)?;
                }
            }
            0xe0 if body.starts(b"JFIF\0", c)? => {
                w.number(
                    "JFIF",
                    "Version",
                    format_args!("{}.{}", body.byte(5, c)?, body.byte(6, c)?),
                    c,
                )?;
                w.put(
                    "JFIF",
                    "Resolution Units",
                    match body.byte(7, c)? {
                        0 => "none",
                        1 => "inch",
                        2 => "centimetre",
                        _ => "unit",
                    },
                    c,
                )?;
                for (name, offset) in [("X Resolution", 8), ("Y Resolution", 10)] {
                    let value = body.u16(offset, false, c)?;
                    w.number(
                        "JFIF",
                        name,
                        format_args!("{value} dot{}", if value == 1 { "" } else { "s" }),
                        c,
                    )?;
                }
                w.number("JFIF", "Thumbnail Width Pixels", body.byte(12, c)?, c)?;
                w.number("JFIF", "Thumbnail Height Pixels", body.byte(13, c)?, c)?;
            }
            0xe1 if body.starts(b"Exif\0\0", c)? => {
                tiff::extract(body.slice(6, body.len() - 6)?, scratch, w, c)?;
            }
            0xe2 if icc::is_fragment(body, c)? => {
                // The first scan handles all fragments. Later recognized fragments
                // must still match this arm rather than falling through to Partial.
                #[allow(clippy::collapsible_match)]
                if !icc_done {
                    icc::jpeg(p, scratch, w, c)?;
                    icc_done = true;
                }
            }
            0xed if body.starts(&[0x1c], c)? => {
                iptc::extract(body, w, c)?;
            }
            0xed if body.starts(b"Photoshop 3.0\0", c)? => {
                carriers::photoshop_resources(body.slice(14, body.len() - 14)?, scratch, w, c)?;
            }
            0xfe => {
                w.text(
                    "JPEG Comment",
                    "Comment",
                    &input::text(body, 0, body.len(), false, false, c)?,
                    c,
                )?;
            }
            0xe0..=0xef | 0xc4 | 0xdc => c.partial = true,
            _ => {}
        }
    }
    Err(Failure::Malformed)
}

// Keep chunk validation and dispatch in the same bounded walk.
#[allow(clippy::too_many_lines)]
fn png(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let mut at = 8;
    let mut header = false;
    while at < p.len() {
        c.step()?;
        let len = p.u32(at, false, c)? as usize;
        let chunk = p.array::<4>(at + 4, c)?;
        let end = at
            .checked_add(12)
            .and_then(|n| n.checked_add(len))
            .ok_or(Failure::Malformed)?;
        if end > p.len() {
            return Err(Failure::Malformed);
        }
        let body = p.slice(at + 8, len)?;
        match &chunk {
            b"IHDR" => {
                if header || at != 8 || len != 13 {
                    return Err(Failure::Malformed);
                }
                header = true;
                w.number("PNG-IHDR", "Image Width", body.u32(0, false, c)?, c)?;
                w.number("PNG-IHDR", "Image Height", body.u32(4, false, c)?, c)?;
                w.number("PNG-IHDR", "Bits Per Sample", body.byte(8, c)?, c)?;
                w.put(
                    "PNG-IHDR",
                    "Color Type",
                    match body.byte(9, c)? {
                        0 => "Greyscale",
                        2 => "True Color",
                        3 => "Indexed Color",
                        4 => "Greyscale with Alpha",
                        6 => "True Color with Alpha",
                        _ => return Err(Failure::Malformed),
                    },
                    c,
                )?;
                let compression = body.byte(10, c)?;
                let filter = body.byte(11, c)?;
                let interlace = body.byte(12, c)?;
                if compression != 0 || filter != 0 || interlace > 1 {
                    return Err(Failure::Malformed);
                }
                w.put("PNG-IHDR", "Compression Type", "Deflate", c)?;
                w.put("PNG-IHDR", "Filter Method", "Adaptive", c)?;
                w.put(
                    "PNG-IHDR",
                    "Interlace Method",
                    if interlace == 0 {
                        "No Interlace"
                    } else {
                        "Adam7 Interlace"
                    },
                    c,
                )?;
            }
            b"PLTE" => {
                w.number("PNG-PLTE", "Palette Size", len / 3, c)?;
            }
            b"tRNS" => {
                w.put("PNG-tRNS", "Palette Has Transparency", "1", c)?;
            }
            b"pHYs" => {
                w.number("PNG-pHYs", "Pixels Per Unit X", body.u32(0, false, c)?, c)?;
                w.number("PNG-pHYs", "Pixels Per Unit Y", body.u32(4, false, c)?, c)?;
                w.put(
                    "PNG-pHYs",
                    "Unit Specifier",
                    match body.byte(8, c)? {
                        0 => "Unspecified",
                        1 => "Metres",
                        _ => "Unknown",
                    },
                    c,
                )?;
            }
            b"tEXt" => {
                let mut split = 0;
                while split < len && body.byte(split, c)? != 0 {
                    c.step()?;
                    split += 1;
                }
                if split == len {
                    return Err(Failure::Malformed);
                }
                let keyword = input::text(body, 0, split, true, false, c)?;
                let value = input::text(body, split + 1, len - split - 1, true, false, c)?;
                let mut text = Text::new();
                let _ = write!(text, "{}: {}", keyword.as_str(), value.as_str());
                w.text("PNG-tEXt", "Textual Data", &text, c)?;
            }
            b"IEND" => {
                return if header && len == 0 {
                    Ok(())
                } else {
                    Err(Failure::Malformed)
                };
            }
            b"IDAT" => {}
            _ => c.partial = true,
        }
        at = end;
    }
    Err(Failure::Malformed)
}

fn bmp(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let size = p.u32(14, true, c)?;
    w.put("BMP Header", "Bitmap type", "Standard", c)?;
    w.number("BMP Header", "Header Size", size, c)?;
    if size == 12 {
        for (name, at) in [
            ("Image Width", 18),
            ("Image Height", 20),
            ("Planes", 22),
            ("Bits Per Pixel", 24),
        ] {
            w.number("BMP Header", name, p.u16(at, true, c)?, c)?;
        }
    } else if size >= 40 {
        p.slice(14, size as usize)?;
        for (name, at) in [
            ("Image Width", 18),
            ("Image Height", 22),
            ("X Pixels per Meter", 38),
            ("Y Pixels per Meter", 42),
        ] {
            w.number("BMP Header", name, p.u32(at, true, c)? as i32, c)?;
        }
        w.number("BMP Header", "Planes", p.u16(26, true, c)?, c)?;
        w.number("BMP Header", "Bits Per Pixel", p.u16(28, true, c)?, c)?;
        for (name, at) in [("Palette Colour Count", 46), ("Important Colour Count", 50)] {
            w.number("BMP Header", name, p.u32(at, true, c)?, c)?;
        }
        let compression = p.u32(30, true, c)?;
        let name = match compression {
            0 => "None",
            1 => "RLE 8-bit/pixel",
            2 => "RLE 4-bit/pixel",
            3 => "Bit Fields",
            4 => "JPEG",
            5 => "PNG",
            6 => "RGBA Bit Fields",
            _ => "Unknown",
        };
        w.put("BMP Header", "Compression", name, c)?;
        if size > 40 {
            c.partial = true;
        }
    } else {
        return Err(Failure::Malformed);
    }
    Ok(())
}
fn gif(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let packed = p.byte(10, c)?;
    w.text(
        "GIF Header",
        "GIF Format Version",
        &input::text(p, 3, 3, true, false, c)?,
        c,
    )?;
    w.number("GIF Header", "Image Width", p.u16(6, true, c)?, c)?;
    w.number("GIF Header", "Image Height", p.u16(8, true, c)?, c)?;
    w.number(
        "GIF Header",
        "Color Table Size",
        1u16 << ((packed & 7) + 1),
        c,
    )?;
    w.number("GIF Header", "Bits per Pixel", ((packed >> 4) & 7) + 1, c)?;
    w.put(
        "GIF Header",
        "Is Color Table Sorted",
        if packed & 8 != 0 { "true" } else { "false" },
        c,
    )?;
    w.put(
        "GIF Header",
        "Has Global Color Table",
        if packed & 128 != 0 { "true" } else { "false" },
        c,
    )?;
    w.number("GIF Header", "Background Color Index", p.byte(11, c)?, c)?;
    let aspect = p.byte(12, c)?;
    if aspect != 0 {
        w.number(
            "GIF Header",
            "Pixel Aspect Ratio",
            (f64::from(aspect) + 15.0) / 64.0,
            c,
        )?;
    }
    // Image/control/comment/application directories follow the header; next checkpoint.
    if p.len() > 13 {
        c.partial = true;
    }
    Ok(())
}
fn ico(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    let typ = p.u16(2, true, c)?;
    let count = usize::from(p.u16(4, true, c)?);
    if count == 0 {
        return Err(Failure::Malformed);
    }
    p.slice(6, count * 16)?;
    for n in 0..count {
        c.step()?;
        let at = 6 + n * 16;
        w.put(
            "ICO",
            "Image Type",
            if typ == 1 { "Icon" } else { "Cursor" },
            c,
        )?;
        for (name, offset) in [("Image Width", 0), ("Image Height", 1)] {
            let v = p.byte(at + offset, c)?;
            w.number(
                "ICO",
                name,
                format_args!("{} pixels", if v == 0 { 256 } else { u16::from(v) }),
                c,
            )?;
        }
        let palette = p.byte(at + 2, c)?;
        if palette == 0 {
            w.put("ICO", "Colour Palette Size", "No palette", c)?;
        } else {
            w.number(
                "ICO",
                "Colour Palette Size",
                format_args!("{palette} colour{}", if palette == 1 { "" } else { "s" }),
                c,
            )?;
        }
        for (name, offset) in if typ == 1 {
            [("Colour Planes", 4), ("Bits Per Pixel", 6)]
        } else {
            [("Hotspot X", 4), ("Hotspot Y", 6)]
        } {
            w.number("ICO", name, p.u16(at + offset, true, c)?, c)?;
        }
        w.number("ICO", "Image Size Bytes", p.u32(at + 8, true, c)?, c)?;
        w.number("ICO", "Image Offset Bytes", p.u32(at + 12, true, c)?, c)?;
    }
    Ok(())
}
fn pcx(p: Prefix<'_>, w: &mut Writer<'_>, c: &mut Control<'_>) -> Result<()> {
    p.slice(0, 74)?;
    let version = match p.byte(1, c)? {
        0 => "2.5 with fixed EGA palette information",
        2 => "2.8 with modifiable EGA palette information",
        3 => "2.8 without palette information (default palette)",
        4 => "PC Paintbrush for Windows",
        5 => "3.0 or better",
        _ => "Unknown",
    };
    w.put("PCX", "Version", version, c)?;
    w.number("PCX", "Bits Per Pixel", p.byte(3, c)?, c)?;
    for (name, at) in [
        ("X Min", 4),
        ("Y Min", 6),
        ("X Max", 8),
        ("Y Max", 10),
        ("Horizontal DPI", 12),
        ("Vertical DPI", 14),
        ("Bytes Per Line", 66),
    ] {
        w.number("PCX", name, p.u16(at, true, c)?, c)?;
    }
    w.put("PCX", "Palette", "[48 values]", c)?;
    let planes = p.byte(65, c)?;
    w.put(
        "PCX",
        "Color Planes",
        match planes {
            3 => "24-bit color",
            4 => "16 colors",
            _ => "Unknown",
        },
        c,
    )?;
    let palette = p.u16(68, true, c)?;
    if palette != 0 {
        w.put(
            "PCX",
            "Palette Type",
            match palette {
                1 => "Color or B&W",
                2 => "Grayscale",
                _ => "Unknown",
            },
            c,
        )?;
    }
    for (name, at) in [("H Scr Size", 70), ("V Scr Size", 72)] {
        let n = p.u16(at, true, c)?;
        if n != 0 {
            w.number("PCX", name, n, c)?;
        }
    }
    Ok(())
}
