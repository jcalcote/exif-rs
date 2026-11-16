// Fixture helpers intentionally accept borrowed FourCC byte literals.
#![allow(clippy::trivially_copy_pass_by_ref)]
use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

fn field_pair(field: Field<'_>) -> (String, String) {
    (
        format!("{}.{}", field.directory, field.name),
        field.value.to_owned(),
    )
}
fn run(bytes: &[u8]) -> (Status, Diagnostic, BTreeMap<String, String>) {
    let mut workspace = vec![0; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let parts = [bytes];
    let result = Extractor::default()
        .extract(
            Prefix::new(&parts, bytes.len()).unwrap(),
            &mut workspace,
            &mut output,
            &AtomicBool::new(false),
        )
        .unwrap();
    (
        result.status(),
        result.diagnostic(),
        result.fields().map(field_pair).collect(),
    )
}
fn jpeg() -> Vec<u8> {
    // Independently constructed 3-component baseline SOF, followed by end marker.
    vec![
        0xff, 0xd8, 0xff, 0xc0, 0, 17, 8, 0, 12, 0, 24, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1,
        0xff, 0xd9,
    ]
}
fn tiff() -> Vec<u8> {
    // LE IFD0: image width=640, orientation=6, Model string="Camera".
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    bytes.extend_from_slice(&3u16.to_le_bytes());
    for (tag, typ, count, value) in [
        (0x100u16, 4u16, 1u32, 640u32),
        (0x112, 3, 1, 6),
        (0x110, 2, 7, 50),
    ] {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&typ.to_le_bytes());
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(b"Camera\0");
    bytes
}
fn riff_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(body);
    if body.len() & 1 != 0 {
        bytes.push(0);
    }
    bytes
}
fn riff(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let size = 4 + chunks.iter().map(Vec::len).sum::<usize>();
    let mut bytes = b"RIFF".to_vec();
    bytes.extend_from_slice(&u32::try_from(size).unwrap().to_le_bytes());
    bytes.extend_from_slice(kind);
    for chunk in chunks {
        bytes.extend_from_slice(chunk);
    }
    bytes
}
fn png_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
    bytes.extend_from_slice(id);
    bytes.extend_from_slice(body);
    // Metadata-extractor does not validate CRCs; structural extraction does not either.
    bytes.extend_from_slice(&[0; 4]);
    bytes
}
fn png() -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.extend(png_chunk(
        b"IHDR",
        &[0, 0, 0, 24, 0, 0, 0, 12, 8, 2, 0, 0, 0],
    ));
    bytes.extend(png_chunk(b"tEXt", b"Title\0A sample"));
    bytes.extend(png_chunk(b"IEND", &[]));
    bytes
}

#[test]
fn plans_check_input_and_include_complete_structured_index() {
    let extractor = Extractor::default();
    let plan = extractor.plan(usize::MAX).unwrap();
    assert_eq!(plan.input_bytes, usize::MAX);
    assert_eq!(plan.contiguous_copy_bytes, 0);
    assert_eq!(plan.workspace_bytes, SCRATCH_BYTES);
    assert_eq!(plan.output_bytes, MAX_OUTPUT_BYTES);
    assert_eq!(plan.fixed_index_bytes, MAX_ENTRIES * 12);
    for limits in [
        Limits {
            max_entries: MAX_ENTRIES + 1,
            output_bytes: MAX_OUTPUT_BYTES,
        },
        Limits {
            max_entries: 1,
            output_bytes: 11,
        },
        Limits {
            max_entries: 0,
            output_bytes: MAX_OUTPUT_BYTES + 1,
        },
    ] {
        assert_eq!(
            Extractor::new(limits).unwrap_err(),
            ExtractionError::InvalidLimits
        );
    }
    assert!(
        Extractor::new(Limits {
            max_entries: 0,
            output_bytes: 0
        })
        .is_ok()
    );
}
#[test]
fn invalid_prefix_and_insufficient_admission_fail_before_parsing() {
    assert_eq!(
        Prefix::new(&[b"abc"], 2).unwrap_err(),
        ExtractionError::InvalidPrefix
    );
    assert_eq!(
        Prefix::new(&vec![&b""[..]; MAX_SEGMENTS + 1], 0).unwrap_err(),
        ExtractionError::InvalidPrefix
    );
    let data = jpeg();
    let parts = [&data[..]];
    let prefix = Prefix::new(&parts, data.len()).unwrap();
    let mut scratch = vec![0; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    assert_eq!(
        Extractor::default()
            .extract(
                prefix,
                &mut scratch[..SCRATCH_BYTES - 1],
                &mut output,
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        ExtractionError::InsufficientStorage
    );
    assert_eq!(
        Extractor::default()
            .extract(
                prefix,
                &mut scratch,
                &mut output[..MAX_OUTPUT_BYTES - 1],
                &AtomicBool::new(false)
            )
            .unwrap_err(),
        ExtractionError::InsufficientStorage
    );
}
#[test]
fn jpeg_has_semantic_header_descriptions_and_source_is_immutable() {
    let bytes = jpeg();
    let original = bytes.clone();
    let (status, _, fields) = run(&bytes);
    assert_eq!(status, Status::Complete);
    assert_eq!(fields["JPEG.Image Width"], "24 pixels");
    assert_eq!(fields["JPEG.Image Height"], "12 pixels");
    assert_eq!(fields["JPEG.Compression Type"], "Baseline");
    assert_eq!(fields["File Type.Detected File Type Name"], "JPEG");
    assert_eq!(bytes, original);
}
#[test]
fn standard_tiff_names_values_and_orientation_use_semantic_contract() {
    let (status, _, fields) = run(&tiff());
    assert_eq!(status, Status::Complete);
    assert_eq!(fields["Exif IFD0.Image Width"], "640 pixels");
    assert_eq!(fields["Exif IFD0.Model"], "Camera");
    assert_eq!(
        fields["Exif IFD0.Orientation"],
        "Right side, top (Rotate 90 CW)"
    );
}
#[test]
fn segmented_input_matches_every_boundary_for_multiple_families() {
    for bytes in [
        jpeg(),
        tiff(),
        png(),
        riff(
            b"WEBP",
            &[riff_chunk(b"VP8X", &[16, 0, 0, 0, 23, 0, 0, 11, 0, 0])],
        ),
    ] {
        let expected = run(&bytes);
        for split in 0..=bytes.len() {
            let parts = [&bytes[..split], &[][..], &bytes[split..]];
            let mut scratch = vec![0; SCRATCH_BYTES];
            let mut output = vec![0; MAX_OUTPUT_BYTES];
            let result = Extractor::default()
                .extract(
                    Prefix::new(&parts, bytes.len()).unwrap(),
                    &mut scratch,
                    &mut output,
                    &AtomicBool::new(false),
                )
                .unwrap();
            let fields = result.fields().map(field_pair).collect();
            assert_eq!(
                (result.status(), result.diagnostic(), fields),
                expected,
                "split {split}"
            );
        }
    }
}
#[test]
fn png_parses_header_and_plain_text_but_reports_compressed_metadata_gap() {
    let (status, _, fields) = run(&png());
    assert_eq!(status, Status::Complete);
    assert_eq!(fields["PNG-IHDR.Color Type"], "True Color");
    assert_eq!(fields["PNG-tEXt.Textual Data"], "Title: A sample");
    let mut bytes = png();
    bytes.truncate(bytes.len() - 12);
    bytes.extend(png_chunk(b"zTXt", b"Title\0\0compressed"));
    bytes.extend(png_chunk(b"IEND", &[]));
    assert_eq!(run(&bytes).0, Status::Partial);
}
#[test]
fn simple_image_header_fields_are_extracted() {
    let mut bmp = vec![0; 54];
    bmp[..2].copy_from_slice(b"BM");
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&24u32.to_le_bytes());
    bmp[22..26].copy_from_slice(&12u32.to_le_bytes());
    bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
    bmp[28..30].copy_from_slice(&24u16.to_le_bytes());
    assert_eq!(run(&bmp).2["BMP Header.Image Width"], "24");
    let gif = b"GIF89a\x18\0\x0c\0\x80\0\0";
    assert_eq!(run(gif).2["GIF Header.Has Global Color Table"], "true");
    let mut ico = b"\0\0\x01\0\x01\0".to_vec();
    ico.extend_from_slice(&[0, 12, 0, 0, 1, 0, 24, 0, 0, 0, 0, 0, 22, 0, 0, 0]);
    assert_eq!(run(&ico).2["ICO.Image Width"], "256 pixels");
    let mut pcx = vec![0; 128];
    pcx[0] = 10;
    pcx[1] = 5;
    pcx[2] = 1;
    pcx[3] = 8;
    pcx[65] = 3;
    assert_eq!(run(&pcx).2["PCX.Color Planes"], "24-bit color");
}
#[test]
fn wav_avi_webp_and_mp3_provide_real_audio_video_metadata() {
    let mut fmt = vec![0; 16];
    fmt[..2].copy_from_slice(&1u16.to_le_bytes());
    fmt[2..4].copy_from_slice(&2u16.to_le_bytes());
    fmt[4..8].copy_from_slice(&48000u32.to_le_bytes());
    fmt[8..12].copy_from_slice(&192_000_u32.to_le_bytes());
    fmt[12..14].copy_from_slice(&4u16.to_le_bytes());
    fmt[14..16].copy_from_slice(&16u16.to_le_bytes());
    let bytes = riff(
        b"WAVE",
        &[riff_chunk(b"fmt ", &fmt), riff_chunk(b"data", &[0; 4])],
    );
    let fields = run(&bytes).2;
    assert_eq!(fields["WAV.Channels"], "2");
    assert_eq!(fields["WAV.Samples Per Second"], "48000");
    assert_eq!(fields["WAV.Bits Per Sample"], "16");
    let mut avih = vec![0; 40];
    avih[24..28].copy_from_slice(&2u32.to_le_bytes());
    avih[32..36].copy_from_slice(&640u32.to_le_bytes());
    avih[36..40].copy_from_slice(&480u32.to_le_bytes());
    let mut strh = vec![0; 36];
    strh[..4].copy_from_slice(b"vids");
    strh[4..8].copy_from_slice(b"MJPG");
    strh[20..24].copy_from_slice(&1u32.to_le_bytes());
    strh[24..28].copy_from_slice(&30u32.to_le_bytes());
    strh[32..36].copy_from_slice(&60u32.to_le_bytes());
    let bytes = riff(
        b"AVI ",
        &[riff_chunk(b"avih", &avih), riff_chunk(b"strh", &strh)],
    );
    assert_eq!(run(&bytes).2["AVI.Frames Per Second"], "30");
    assert_eq!(run(&bytes).2["AVI.Video Codec"], "MJPG");
    let bytes = riff(
        b"WEBP",
        &[riff_chunk(b"VP8X", &[16, 0, 0, 0, 23, 0, 0, 11, 0, 0])],
    );
    assert_eq!(run(&bytes).2["WebP.Image Width"], "24");
    assert_eq!(run(&bytes).2["WebP.Has Alpha"], "true");
    let fields = run(&[0xff, 0xfb, 0x90, 0x00]).2;
    assert_eq!(fields["MP3.ID"], "MPEG-1");
    assert_eq!(fields["MP3.Bitrate"], "128");
    assert_eq!(fields["MP3.Frequency"], "44100");
}
#[test]
fn malformed_carriers_are_not_recognition_only_success() {
    for input in [
        &b"8BPS\0\x01"[..],
        &b"FUJIFILMCCD-RAW"[..],
        &b"\0\0\0\x0cftypisom"[..],
    ] {
        let (status, _, fields) = run(input);
        assert_eq!(status, Status::ReaderError);
        assert!(fields.is_empty());
    }
    let (status, diagnostic, fields) = run(b"%!PS-Adobe-3.0");
    assert_eq!(status, Status::Partial);
    assert_eq!(diagnostic, Diagnostic::UnsupportedMetadata);
    assert!(fields.contains_key("File Type.Detected File Type Name"));
    let (status, diagnostic, fields) = run(b"%PDF-1.7");
    assert_eq!(status, Status::Partial);
    assert_eq!(diagnostic, Diagnostic::UnsupportedMetadata);
    for (name, value) in [
        ("Detected File Type Name", "PDF"),
        ("Detected File Type Long Name", "Portable Document Format"),
        ("Detected MIME Type", "application/pdf"),
        ("Expected File Name Extension", "pdf"),
    ] {
        assert_eq!(
            fields.get(&format!("File Type.{name}")).map(String::as_str),
            Some(value)
        );
    }
    assert_eq!(fields.len(), 4);
    assert_eq!(run(b"unrecognized").0, Status::ReaderError);
}

#[test]
fn malformed_offsets_cycles_and_truncated_containers_fail_without_partial_commit() {
    let mut cycle = tiff();
    cycle[46..50].copy_from_slice(&8u32.to_le_bytes());
    for bytes in [
        cycle,
        vec![0xff, 0xd8, 0xff, 0xe1, 0xff, 0xff],
        b"II\x2a\0\xff\xff\xff\xff".to_vec(),
    ] {
        let (status, _, fields) = run(&bytes);
        assert_eq!(status, Status::ReaderError);
        assert!(fields.is_empty());
    }
}
#[test]
fn output_limit_has_explicit_status_and_no_partial_fields() {
    let bytes = jpeg();
    let parts = [&bytes[..]];
    let prefix = Prefix::new(&parts, bytes.len()).unwrap();
    let cancel = AtomicBool::new(false);
    let mut scratch = vec![0; SCRATCH_BYTES];
    for limits in [
        Limits {
            max_entries: 0,
            output_bytes: 1024,
        },
        Limits {
            max_entries: 512,
            output_bytes: MAX_ENTRIES * 12,
        },
    ] {
        let mut output = vec![0; limits.output_bytes];
        let extractor = Extractor::new(limits).unwrap();
        let result = extractor
            .extract(prefix, &mut scratch, &mut output, &cancel)
            .unwrap();
        assert_eq!(result.status(), Status::ResourceLimit);
        assert!(result.fields().next().is_none());
        assert!(matches!(
            result.diagnostic(),
            Diagnostic::EntryLimit | Diagnostic::OutputLimit
        ));
    }
}
#[test]
fn packed_map_replaces_duplicates_without_leaking_old_value_space() {
    let mut bytes = [0; 100];
    let cancel = AtomicBool::new(false);
    let mut control = Control {
        cancel: &cancel,
        remaining: MAX_STEPS,
        partial: false,
    };
    let mut writer = packed::Writer::new(&mut bytes, 2);
    writer.put("A", "B", "1234567890", &mut control).unwrap();
    writer.put("A", "C", "other", &mut control).unwrap();
    writer.put("A", "B", "new", &mut control).unwrap();
    let result = writer.finish(Status::Complete, Diagnostic::None);
    assert_eq!(
        result.fields().collect::<Vec<_>>(),
        vec![
            Field {
                directory: "A",
                name: "B",
                value: "new"
            },
            Field {
                directory: "A",
                name: "C",
                value: "other"
            }
        ]
    );
    assert_eq!(result.used_output_bytes(), 24 + 2 + 3 + 2 + 5);
}
#[test]
fn utf8_text_limit_is_explicit_and_long_unicode_is_preserved() {
    let mut bytes = vec![0; 8192];
    let cancel = AtomicBool::new(false);
    let mut control = Control {
        cancel: &cancel,
        remaining: MAX_STEPS,
        partial: false,
    };
    let mut writer = packed::Writer::new(&mut bytes, 2);
    let long = "😀".repeat(256);
    writer.put("A", "B", &long, &mut control).unwrap();
    assert_eq!(
        writer.put("A", "B", &"😀".repeat(257), &mut control),
        Err(Failure::Limit(Diagnostic::TextLimit))
    );
    assert_eq!(
        writer.put(&"D".repeat(1025), "tag", "value", &mut control),
        Err(Failure::Limit(Diagnostic::TextLimit))
    );
    assert_eq!(
        writer.put("A", &"N".repeat(1025), "value", &mut control),
        Err(Failure::Limit(Diagnostic::TextLimit))
    );
    let result = writer.finish(Status::Complete, Diagnostic::None);
    assert_eq!(result.fields().next().unwrap().value, long);
}
#[test]
fn cancellation_and_operation_budget_are_causal() {
    let cancel = AtomicBool::new(true);
    let result =
        Extractor::default().extract(Prefix::new(&[], 0).unwrap(), &mut [], &mut [], &cancel);
    assert_eq!(result.unwrap_err(), ExtractionError::Cancelled);
    let cancel = AtomicBool::new(false);
    let mut c = Control {
        cancel: &cancel,
        remaining: 1,
        partial: false,
    };
    c.step().unwrap();
    assert_eq!(c.step(), Err(Failure::Limit(Diagnostic::StepLimit)));
    let mut c = Control {
        cancel: &cancel,
        remaining: 100,
        partial: false,
    };
    c.step().unwrap();
    cancel.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(c.step(), Err(Failure::Cancelled));
    cancel.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut c = Control {
        cancel: &cancel,
        remaining: 100,
        partial: false,
    };
    assert_eq!(
        c.depth(MAX_NESTING),
        Err(Failure::Limit(Diagnostic::NestingLimit))
    );
}
#[test]
fn output_survives_source_and_workspace_retirement() {
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let result;
    {
        let source = tiff();
        let parts = [&source[..]];
        let mut scratch = vec![0; SCRATCH_BYTES];
        result = Extractor::default()
            .extract(
                Prefix::new(&parts, source.len()).unwrap(),
                &mut scratch,
                &mut output,
                &AtomicBool::new(false),
            )
            .unwrap();
    }
    assert!(result.fields().any(|field| field.directory == "Exif IFD0"
        && field.name == "Model"
        && field.value == "Camera"));
}

include!("../resources/reference-fixtures.rs");

#[test]
fn pinned_java_reference_maps_match_supported_fields() {
    let mut differences = Vec::new();
    for (name, bytes, expected) in UPSTREAM_REFERENCE {
        let (status, _, mut actual) = run(bytes);
        let mut expected: BTreeMap<_, _> = expected
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        tiff_type_difference(&mut actual, &mut expected);
        if *name == "jpeg" {
            assert_synthetic_jpeg_components(bytes, &mut actual, &expected);
        }
        // Java's RIFF reader compares an absolute stream position to a relative
        // section size and misses this tiny final data chunk. Preserve the useful
        // semantic duration instead of copying that boundary bug; verify it.
        if *name == "wav" && !expected.contains_key("WAV.Duration") {
            assert_eq!(actual.remove("WAV.Duration").as_deref(), Some("00:00:00"));
        }
        if status == Status::Partial {
            assert!(
                !actual.is_empty(),
                "{name}: a coverage gap must retain independently verifiable fields"
            );
            for (key, value) in actual {
                assert_eq!(expected.get(&key), Some(&value), "{name}: {key}");
            }
        } else {
            assert_eq!(status, Status::Complete, "{name}");
            if actual != expected {
                differences.push(format!("{name}: actual={actual:?}, expected={expected:?}"));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn exact_output_capacity_passes_and_minus_one_is_explicit() {
    let input = b"%PDF-1.7";
    let parts = [&input[..]];
    let prefix = Prefix::new(&parts, input.len()).unwrap();
    let cancel = AtomicBool::new(false);
    let mut scratch = vec![0; SCRATCH_BYTES];
    let mut generous = vec![0; MAX_OUTPUT_BYTES];
    let extractor = Extractor::new(Limits {
        max_entries: 4,
        output_bytes: MAX_OUTPUT_BYTES,
    })
    .unwrap();
    let used = extractor
        .extract(prefix, &mut scratch, &mut generous, &cancel)
        .unwrap()
        .used_output_bytes();
    for (size, expected) in [(used, Status::Partial), (used - 1, Status::ResourceLimit)] {
        let extractor = Extractor::new(Limits {
            max_entries: 4,
            output_bytes: size,
        })
        .unwrap();
        let mut output = vec![0; size];
        assert_eq!(
            extractor
                .extract(prefix, &mut scratch, &mut output, &cancel)
                .unwrap()
                .status(),
            expected
        );
    }
}
#[test]
fn actual_ifd_chain_obeys_nesting_limit() {
    let mut bytes = b"II\x2a\0\x08\0\0\0".to_vec();
    for i in 0..=MAX_NESTING {
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(8 + (i + 1) * 6).unwrap().to_le_bytes());
    }
    let (status, diagnostic, fields) = run(&bytes);
    assert_eq!(status, Status::ResourceLimit);
    assert_eq!(diagnostic, Diagnostic::NestingLimit);
    assert!(fields.is_empty());
}

#[test]
fn descriptor_reborrows_after_input_and_extraction_retirement() {
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let descriptor = {
        let source = tiff();
        let parts = [&source[..]];
        let mut scratch = vec![0; SCRATCH_BYTES];
        let extraction = Extractor::default()
            .extract(
                Prefix::new(&parts, source.len()).unwrap(),
                &mut scratch,
                &mut output,
                &AtomicBool::new(false),
            )
            .unwrap();
        extraction.descriptor()
    };
    assert_eq!(descriptor.status(), Status::Complete);
    assert_eq!(descriptor.diagnostic(), Diagnostic::None);
    assert!(
        descriptor
            .fields(&output)
            .unwrap()
            .any(|field| field.directory == "Exif IFD0"
                && field.name == "Model"
                && field.value == "Camera")
    );
    assert_eq!(
        descriptor
            .fields(&output[..descriptor.used_output_bytes() - 1])
            .unwrap_err(),
        ExtractionError::InsufficientStorage
    );
}
#[test]
fn descriptor_rejects_corrupted_offsets_lengths_overlap_and_utf8() {
    let source = jpeg();
    let parts = [&source[..]];
    let mut scratch = vec![0; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let descriptor = Extractor::default()
        .extract(
            Prefix::new(&parts, source.len()).unwrap(),
            &mut scratch,
            &mut output,
            &AtomicBool::new(false),
        )
        .unwrap()
        .descriptor();
    let original = output.clone();
    for (at, bytes) in [
        (0, vec![255, 255, 255, 255]),
        (4, vec![255, 255]),
        (6, vec![255, 255]),
        (8, vec![255, 255]),
        (10, vec![1, 0]),
        (12, original[..12].to_vec()),
    ] {
        output.copy_from_slice(&original);
        output[at..at + bytes.len()].copy_from_slice(&bytes);
        assert_eq!(
            descriptor.fields(&output).unwrap_err(),
            ExtractionError::InvalidPackedOutput
        );
    }
    output.copy_from_slice(&original);
    let offset = u32::from_le_bytes(output[..4].try_into().unwrap()) as usize;
    output[offset] = 0xff;
    assert_eq!(
        descriptor.fields(&output).unwrap_err(),
        ExtractionError::InvalidPackedOutput
    );
    output.copy_from_slice(&original);
    let key_len = usize::from(u16::from_le_bytes(output[4..6].try_into().unwrap()));
    output[offset + key_len] = 0xff;
    assert_eq!(
        descriptor.fields(&output).unwrap_err(),
        ExtractionError::InvalidPackedOutput
    );
}

include!("../resources/carrier-inputs.rs");

fn carrier(name: &str) -> &'static [u8] {
    CARRIER_INPUTS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap()
        .1
}
#[test]
fn next_carriers_extract_semantic_fields_and_offsets() {
    for (name, key, value) in [
        ("orf-le", "Exif IFD0.Image Width", "640 pixels"),
        ("orf-be", "Exif IFD0.Image Width", "640 pixels"),
        ("rw2", "PanasonicRaw Exif IFD0.Sensor Width", "4000"),
        ("cr2", "Exif IFD0.Image Width", "640 pixels"),
        ("raf", "JPEG.Image Width", "24 pixels"),
        ("psd", "PSD Header.Image Width", "24 pixels"),
        ("psd-exif", "Exif IFD0.Model", "Camera"),
        ("eps", "EPS.Image Width", "24 pixels"),
        ("eps-continuation", "EPS.Title", "First second"),
        ("eps-tiff", "Exif IFD0.Model", "Camera"),
        ("mp4", "MP4.Duration in Seconds", "00:00:03"),
        ("mp4-v1", "MP4.Duration in Seconds", "00:00:02"),
        ("mov", "QuickTime.Next Track ID", "2"),
        ("heif", "HEIF.Width", "24 pixels"),
    ] {
        let (status, _, fields) = run(carrier(name));
        assert!(
            matches!(status, Status::Complete | Status::Partial),
            "{name}: {status:?}"
        );
        assert_eq!(
            fields.get(key).map(String::as_str),
            Some(value),
            "{name}: {key}"
        );
    }
    let (_, _, fields) = run(carrier("eps-wmf"));
    assert_eq!(fields["EPS.WMF Preview Size"], "4");
    let (_, _, fields) = run(carrier("heif"));
    assert_eq!(fields["HEIF.Rotation"], "90 degrees");
    assert_eq!(fields["HEIF.Bits Per Channel"], "8 8 8");
    let (status, _, fields) = run(carrier("heif-no-handler"));
    assert_eq!(status, Status::Partial);
    assert!(!fields.contains_key("HEIF.Width"));
}
#[test]
fn carrier_offset_and_size_controls_are_malformed_or_explicitly_limited() {
    for (name, bytes) in CARRIER_INPUTS {
        if name.starts_with("bad-") {
            let (status, _, fields) = run(bytes);
            assert_eq!(status, Status::ReaderError, "{name}");
            assert!(
                fields.is_empty(),
                "{name}: malformed data must not escape as complete fields"
            );
        }
    }
    let (status, diagnostic, fields) = run(carrier("deep-mp4"));
    assert_eq!(status, Status::ResourceLimit);
    assert_eq!(diagnostic, Diagnostic::NestingLimit);
    assert!(fields.is_empty());
}
#[test]
fn nested_carriers_are_segmentation_invariant_and_source_immutable() {
    for (name, bytes) in CARRIER_INPUTS {
        if name.starts_with("bad-") || *name == "deep-mp4" {
            continue;
        }
        let original = bytes.to_vec();
        let expected = run(bytes);
        let mut scratch = vec![0; SCRATCH_BYTES];
        let mut output = vec![0; MAX_OUTPUT_BYTES];
        for split in 0..=bytes.len() {
            let parts = [&bytes[..split], &bytes[split..]];
            let result = Extractor::default()
                .extract(
                    Prefix::new(&parts, bytes.len()).unwrap(),
                    &mut scratch,
                    &mut output,
                    &AtomicBool::new(false),
                )
                .unwrap();
            let actual = result.fields().map(field_pair).collect();
            assert_eq!(
                (result.status(), result.diagnostic(), actual),
                expected,
                "{name}, split {split}"
            );
        }
        assert_eq!(*bytes, &original[..], "{name}: source changed");
    }
}

include!("../resources/carrier-reference.rs");
#[test]
fn carrier_java_reference_fields_and_malformed_controls_are_explicit() {
    let mut differences = Vec::new();
    for (name, expected) in CARRIER_REFERENCE {
        let (status, _, mut actual) = run(carrier(name));
        let mut expected: BTreeMap<_, _> = expected
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        tiff_type_difference(&mut actual, &mut expected);
        if *name == "raf" {
            assert_synthetic_jpeg_components(carrier(name), &mut actual, &expected);
        }
        if name.starts_with("bad-") {
            assert_eq!(status, Status::ReaderError, "{name}");
            assert!(actual.is_empty());
            // Java recovers directory/header fields for some malformed tails;
            // Rust rejects malformed offsets instead of publishing them complete.
            match *name {
                "bad-orf" | "bad-raf" | "bad-eps-offset" => {
                    assert!(expected.is_empty());
                }
                "bad-psd-offset" | "bad-psd-resource" => {
                    assert_eq!(
                        expected.get("PSD Header.Image Width").map(String::as_str),
                        Some("24 pixels")
                    );
                }
                "bad-mp4-box" => {
                    assert_eq!(
                        expected.get("MP4.Minor Version").map(String::as_str),
                        Some("0")
                    );
                }
                "bad-heif-property" => {
                    assert_eq!(
                        expected.get("HEIF.Major Brand").map(String::as_str),
                        Some("heic")
                    );
                    assert!(!expected.contains_key("HEIF.Width"));
                }
                _ => panic!("unclassified malformed Java control {name}"),
            }
        } else if *name == "deep-mp4" {
            assert_eq!(status, Status::ResourceLimit);
            assert!(actual.is_empty());
            assert!(expected.contains_key("MP4.Minor Version"));
        } else if status == Status::Partial {
            assert!(!actual.is_empty(), "{name}");
            for (key, value) in actual {
                if expected.get(&key) != Some(&value) {
                    differences.push(format!(
                        "{name}: {key}: Rust={value:?}, Java={:?}",
                        expected.get(&key)
                    ));
                }
            }
        } else {
            assert_eq!(status, Status::Complete, "{name}");
            if actual != expected {
                differences.push(format!("{name}: Rust={actual:?}, Java={expected:?}"));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

include!("../resources/structured-inputs.rs");
fn structured(name: &str) -> &'static [u8] {
    STRUCTURED_INPUTS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap()
        .1
}
#[test]
fn selected_iptc_fields_repeats_dates_and_utf8_are_semantic() {
    for name in ["jpeg-iptc-basic", "psd-iptc", "tiff-iptc"] {
        let (status, _, fields) = run(structured(name));
        assert_eq!(status, Status::Complete, "{name}");
        for (key, value) in [
            ("Object Name", "Scene"),
            ("Application Record Version", "2"),
            ("Urgency", "5"),
            ("Keywords", "alpha;beta"),
            ("Date Created", "2024:01:02"),
            ("Time Created", "03:04:05+0100"),
            ("By-line", "Sam"),
            ("Headline", "Example"),
            ("Caption/Abstract", "Sample caption"),
        ] {
            assert_eq!(
                fields.get(&format!("IPTC.{key}")).map(String::as_str),
                Some(value),
                "{name}: {key}"
            );
        }
    }
    let (_, _, fields) = run(structured("jpeg-iptc-utf8"));
    assert_eq!(fields["IPTC.Coded Character Set"], "UTF-8");
    assert_eq!(fields["IPTC.Object Name"], "Résumé 😀");
    let (_, _, fields) = run(structured("jpeg-iptc-repeats"));
    assert_eq!(fields["IPTC.Object Name"], "[17 values]");
    assert_eq!(fields["IPTC.Keywords"], "one;two");
    let (_, _, fields) = run(structured("jpeg-iptc-long"));
    assert_eq!(fields["IPTC.Object Name"].len(), 65);
    assert_eq!(fields["IPTC.By-line"], "Sam");
}
#[test]
fn icc_headers_and_typed_tags_share_borrowed_carrier_paths() {
    for name in ["jpeg-icc", "jpeg-icc-split", "psd-icc", "tiff-icc"] {
        let (status, _, fields) = run(structured(name));
        assert_eq!(status, Status::Complete, "{name}");
        for (key, value) in [
            ("Version", "2.1.0"),
            ("Class", "Display Device"),
            ("Primary Platform", "Microsoft Corporation"),
            ("Rendering Intent", "Media-Relative Colorimetric"),
            ("Profile Date/Time", "2024:01:02 03:04:05"),
            ("XYZ values", "0.5 1 0.25"),
            ("Profile Description", "Synthetic display"),
            ("Profile Copyright", "Public fixture"),
            ("Technology", "dcam"),
        ] {
            assert_eq!(
                fields
                    .get(&format!("ICC Profile.{key}"))
                    .map(String::as_str),
                Some(value),
                "{name}: {key}"
            );
        }
    }
    let (status, _, fields) = run(structured("jpeg-icc-unknown"));
    assert_eq!(status, Status::Partial);
    assert!(fields.contains_key("ICC Profile.Profile Description"));
    assert!(!fields.contains_key("ICC Profile.Red TRC"));
}
#[test]
fn structured_lengths_offsets_and_fragment_coverage_fail_explicitly() {
    for (name, bytes) in STRUCTURED_INPUTS {
        if name.starts_with("bad-") {
            let (status, _, fields) = run(bytes);
            assert_eq!(status, Status::ReaderError, "{name}");
            assert!(fields.is_empty(), "{name}");
        }
    }
}
#[test]
fn icc_fragment_index_uses_only_funded_scratch_and_descriptors() {
    let bytes = structured("jpeg-icc-split");
    let parts = [bytes];
    let prefix = Prefix::new(&parts, bytes.len()).unwrap();
    let mut scratch = vec![0xa5; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let result = Extractor::default()
        .extract(prefix, &mut scratch, &mut output, &AtomicBool::new(false))
        .unwrap();
    assert_eq!(result.status(), Status::Complete);
    assert!(scratch[255 * 16..].iter().all(|byte| *byte == 0xa5));
    assert!(scratch[..255 * 16].iter().any(|byte| *byte != 0xa5));
}
#[test]
fn structured_fields_are_segment_invariant_without_mutating_source() {
    for (name, bytes) in STRUCTURED_INPUTS {
        let original = bytes.to_vec();
        let expected = run(bytes);
        let mut scratch = vec![0; SCRATCH_BYTES];
        let mut output = vec![0; MAX_OUTPUT_BYTES];
        for split in 0..=bytes.len() {
            let parts = [&bytes[..split], &bytes[split..]];
            let result = Extractor::default()
                .extract(
                    Prefix::new(&parts, bytes.len()).unwrap(),
                    &mut scratch,
                    &mut output,
                    &AtomicBool::new(false),
                )
                .unwrap();
            let actual = result.fields().map(field_pair).collect();
            assert_eq!(
                (result.status(), result.diagnostic(), actual),
                expected,
                "{name}, split {split}"
            );
        }
        assert_eq!(*bytes, &original[..]);
    }
}

include!("../resources/structured-reference.rs");
#[test]
fn structured_java_fields_and_malformed_controls_are_explicit() {
    let mut differences = Vec::new();
    for (name, expected) in STRUCTURED_REFERENCE {
        let (status, _, mut actual) = run(structured(name));
        if *name == "jpeg-iptc-long" {
            assert_eq!(actual.remove("IPTC.Object Name").unwrap().len(), 65);
        }
        let mut expected: BTreeMap<_, _> = expected
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
        tiff_type_difference(&mut actual, &mut expected);
        if matches!(
            *name,
            "jpeg-iptc-basic"
                | "jpeg-iptc-utf8"
                | "jpeg-iptc-repeats"
                | "jpeg-iptc-long"
                | "jpeg-icc"
                | "jpeg-icc-split"
                | "jpeg-icc-unknown"
        ) {
            assert_synthetic_jpeg_components(structured(name), &mut actual, &expected);
        }
        if name.starts_with("bad-") {
            assert_eq!(status, Status::ReaderError, "{name}");
            assert!(actual.is_empty());
            match *name {
                "bad-iptc-length" => assert_eq!(
                    expected.get("JPEG.Image Width").map(String::as_str),
                    Some("24 pixels")
                ),
                "bad-icc-offset" | "bad-icc-size" => assert_eq!(
                    expected.get("ICC Profile.Version").map(String::as_str),
                    Some("2.1.0")
                ),
                "bad-icc-duplicate" | "bad-icc-missing" => assert_eq!(
                    expected
                        .get("ICC Profile.Profile Description")
                        .map(String::as_str),
                    Some("Synthetic display")
                ),
                _ => panic!("unexpected malformed structured case {name}"),
            }
        } else if status == Status::Partial {
            assert!(!actual.is_empty(), "{name}");
            for (key, value) in actual {
                if expected.get(&key) != Some(&value) {
                    differences.push(format!(
                        "{name}: {key}: Rust={value:?}, Java={:?}",
                        expected.get(&key)
                    ));
                }
            }
        } else {
            assert_eq!(status, Status::Complete, "{name}");
            if actual != expected {
                differences.push(format!("{name}: Rust={actual:?}, Java={expected:?}"));
            }
        }
    }
    assert!(differences.is_empty(), "{}", differences.join("\n"));
}

#[test]
fn owned_api_has_structured_fields_and_retires_source() {
    let metadata = {
        let source = tiff();
        crate::extract(&source).unwrap()
    };
    assert_eq!(metadata.status(), Status::Complete);
    assert_eq!(metadata.diagnostic(), Diagnostic::None);
    assert!(metadata.fields().any(|field| field.directory == "Exif IFD0"
        && field.name == "Model"
        && field.value == "Camera"));
}
#[test]
fn unknown_and_empty_input_have_explicit_no_field_results() {
    for input in [&b""[..], &b"unknown media"[..]] {
        let metadata = crate::extract(input).unwrap();
        assert_eq!(metadata.status(), Status::ReaderError);
        assert_eq!(metadata.diagnostic(), Diagnostic::UnknownFormat);
        assert_eq!(metadata.fields().count(), 0);
    }
}
#[test]
fn long_comments_are_retained_or_fail_explicitly_without_partial_fields() {
    for (len, expected) in [
        (65, Status::Complete),
        (1024, Status::Complete),
        (1025, Status::ResourceLimit),
    ] {
        let mut source = vec![0xff, 0xd8, 0xff, 0xfe];
        source.extend_from_slice(&u16::try_from(len + 2).unwrap().to_be_bytes());
        source.extend(std::iter::repeat_n(b'a', len));
        source.extend_from_slice(&[0xff, 0xd9]);
        let metadata = crate::extract(&source).unwrap();
        assert_eq!(metadata.status(), expected, "{len}");
        if expected == Status::Complete {
            assert_eq!(
                metadata
                    .fields()
                    .find(|f| f.directory == "JPEG Comment")
                    .unwrap()
                    .value
                    .len(),
                len
            );
        } else {
            assert_eq!(metadata.diagnostic(), Diagnostic::TextLimit);
            assert_eq!(metadata.fields().count(), 0);
        }
    }
}
#[test]
fn descriptor_mutations_and_shortened_storage_never_panic() {
    let source = jpeg();
    let parts = [&source[..]];
    let mut scratch = vec![0; SCRATCH_BYTES];
    let mut output = vec![0; MAX_OUTPUT_BYTES];
    let descriptor = Extractor::default()
        .extract(
            Prefix::new(&parts, source.len()).unwrap(),
            &mut scratch,
            &mut output,
            &AtomicBool::new(false),
        )
        .unwrap()
        .descriptor();
    let used = descriptor.used_output_bytes();
    for len in 0..used {
        assert!(descriptor.fields(&output[..len]).is_err());
    }
    for pos in 0..used {
        let old = output[pos];
        output[pos] ^= 0xff;
        if let Ok(fields) = descriptor.fields(&output) {
            for field in fields {
                std::hint::black_box(field);
            }
        }
        output[pos] = old;
    }
}
#[test]
fn text_layout_and_nested_walk_fit_explicit_storage_envelope() {
    assert!(std::mem::size_of::<packed::Text>() <= MAX_TEXT_BYTES + 32);
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let mut source = b"II\x2a\0\x08\0\0\0".to_vec();
            for i in 0..MAX_NESTING - 1 {
                source.extend_from_slice(&1u16.to_le_bytes());
                source.extend_from_slice(&0x100u16.to_le_bytes());
                source.extend_from_slice(&4u16.to_le_bytes());
                source.extend_from_slice(&1u32.to_le_bytes());
                source.extend_from_slice(&640u32.to_le_bytes());
                source.extend_from_slice(
                    &u32::try_from(if i == MAX_NESTING - 2 {
                        0
                    } else {
                        8 + (i + 1) * 18
                    })
                    .unwrap()
                    .to_le_bytes(),
                );
            }
            assert_eq!(run(&source).0, Status::Complete);
        })
        .unwrap()
        .join()
        .unwrap();
}

// Upstream used a conventional TIFF offset as Sony evidence. Preserve its rows,
// but explicitly assert the independent conventional TIFF classification here.
fn tiff_type_difference(
    actual: &mut BTreeMap<String, String>,
    expected: &mut BTreeMap<String, String>,
) {
    if actual
        .get("File Type.Detected File Type Name")
        .map(String::as_str)
        == Some("TIFF")
        && expected
            .get("File Type.Detected File Type Name")
            .map(String::as_str)
            == Some("ARW")
    {
        for (name, value) in [
            ("Detected File Type Name", "TIFF"),
            ("Detected File Type Long Name", "Tagged Image File Format"),
            ("Expected File Name Extension", "tiff"),
            ("Detected MIME Type", "image/tiff"),
        ] {
            assert_eq!(
                actual.remove(&format!("File Type.{name}")).as_deref(),
                Some(value)
            );
            expected.remove(&format!("File Type.{name}"));
        }
    }
}
#[test]
fn ordinary_tiff_is_not_assumed_to_be_sony_raw() {
    assert_eq!(run(&tiff()).2["File Type.Detected File Type Name"], "TIFF");
    assert_eq!(
        run(carrier("cr2")).2["File Type.Detected File Type Name"],
        "CR2"
    );
    assert_eq!(run(b"%PDF-1.7").0, Status::Partial);
}
#[test]
fn large_borrowed_input_reaches_metadata_past_four_mebibytes() {
    let later = 4 * 1024 * 1024 + 128;
    let mut source = b"II\x2a\0\x08\0\0\0".to_vec();
    source.extend_from_slice(&1u16.to_le_bytes());
    source.extend_from_slice(&0x10fu16.to_le_bytes());
    source.extend_from_slice(&2u16.to_le_bytes());
    source.extend_from_slice(&11u32.to_le_bytes());
    source.extend_from_slice(&u32::try_from(later).unwrap().to_le_bytes());
    source.extend_from_slice(&0u32.to_le_bytes());
    source.resize(later, 0);
    source.extend_from_slice(b"OpenCamera\0");
    let metadata = crate::extract(&source).unwrap();
    assert_eq!(metadata.status(), Status::Complete);
    assert_eq!(
        metadata
            .fields()
            .find(|f| f.directory == "Exif IFD0" && f.name == "Make")
            .unwrap()
            .value,
        "OpenCamera"
    );
    let parts = [&source[..26], &source[26..later], &source[later..]];
    assert_eq!(
        Prefix::new(&parts, source.len()).unwrap().len(),
        source.len()
    );
}
#[test]
fn input_descriptor_and_total_length_bounds_are_independent() {
    let parts = vec![&b""[..]; MAX_SEGMENTS];
    assert!(Prefix::new(&parts, 0).is_ok());
    let too_many = vec![&b""[..]; MAX_SEGMENTS + 1];
    assert!(Prefix::new(&too_many, 0).is_err());
    assert_eq!(
        input::checked_length([usize::MAX, 1].into_iter(), usize::MAX),
        Err(ExtractionError::InvalidPrefix)
    );
    assert_eq!(
        input::checked_length([usize::MAX - 1, 1].into_iter(), usize::MAX),
        Ok(usize::MAX)
    );
}

// These independent fixtures contain the same three-component SOF. Verify the
// literal component IDs, H/V factors and quantization selectors before asserting
// the three additional long descriptions that the historical reference omitted.
// Only explicitly named fixture comparisons call this; all other differences
// remain visible to the complete/subset reference assertions.
fn assert_synthetic_jpeg_components(
    bytes: &[u8],
    actual: &mut BTreeMap<String, String>,
    reference: &BTreeMap<String, String>,
) {
    let sof = [
        0xff, 0xc0, 0x00, 0x11, 0x08, 0x00, 0x0c, 0x00, 0x18, 0x03, 0x01, 0x22, 0x00, 0x02, 0x11,
        0x01, 0x03, 0x11, 0x01,
    ];
    assert!(
        bytes.windows(sof.len()).any(|window| window == sof),
        "known synthetic SOF required"
    );
    for (name, description) in [
        (
            "Component 1",
            "Y component: Quantization table 0, Sampling factors 2 horiz/2 vert",
        ),
        (
            "Component 2",
            "Cb component: Quantization table 1, Sampling factors 1 horiz/1 vert",
        ),
        (
            "Component 3",
            "Cr component: Quantization table 1, Sampling factors 1 horiz/1 vert",
        ),
    ] {
        let key = format!("JPEG.{name}");
        assert!(
            !reference.contains_key(&key),
            "reference rows must remain unchanged"
        );
        assert_eq!(actual.remove(&key).as_deref(), Some(description), "{name}");
    }
}
