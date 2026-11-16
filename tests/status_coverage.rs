//! Independent metadata-only fixtures for encountered unsupported sections.
use exif_inspect::{Diagnostic, Extractor, Field, Prefix, Status};
use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;

fn collect<'a>(fields: impl Iterator<Item = Field<'a>>) -> BTreeMap<(String, String), String> {
    fields
        .map(|field| {
            (
                (field.directory.to_owned(), field.name.to_owned()),
                field.value.to_owned(),
            )
        })
        .collect()
}

fn assert_all_input_paths(
    bytes: &[u8],
    status: Status,
    expected_fields: &[(&str, &str, &str)],
) -> BTreeMap<(String, String), String> {
    let diagnostic = if status == Status::Partial {
        Diagnostic::UnsupportedMetadata
    } else {
        Diagnostic::None
    };
    let owned = exif_inspect::extract(bytes).unwrap();
    assert_eq!(owned.status(), status);
    assert_eq!(owned.diagnostic(), diagnostic);
    let expected = collect(owned.fields());
    for &(directory, name, value) in expected_fields {
        assert_eq!(
            expected
                .get(&(directory.to_owned(), name.to_owned()))
                .map(String::as_str),
            Some(value)
        );
    }

    let original = bytes.to_vec();
    let extractor = Extractor::default();
    let plan = extractor.plan(bytes.len()).unwrap();
    let mut scratch = vec![0; plan.workspace_bytes];
    let mut output = vec![0; plan.output_bytes];
    let cancel = AtomicBool::new(false);
    // Endpoints exercise a single nonempty arena view; every interior boundary
    // exercises the same immutable source as two separately borrowed segments.
    for split in 0..=bytes.len() {
        let parts = [&bytes[..split], &bytes[split..]];
        let result = extractor
            .extract(
                Prefix::new(&parts, bytes.len()).unwrap(),
                &mut scratch,
                &mut output,
                &cancel,
            )
            .unwrap();
        assert_eq!(result.status(), status, "split {split}");
        assert_eq!(result.diagnostic(), diagnostic, "split {split}");
        assert_eq!(collect(result.fields()), expected, "split {split}");
    }
    assert_eq!(bytes, original);
    expected
}

#[test]
fn wav_unknown_info_is_partial_and_does_not_hide_later_title() {
    // Valid mono 8-kHz/16-bit PCM, LIST INFO containing unsupported ISBJ then
    // supported INAM, followed by four sample bytes. RIFF size = 92 - 8.
    let bytes = b"RIFF\x54\0\0\0WAVE\
        fmt \x10\0\0\0\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0\
        LIST\x24\0\0\0INFO\
        ISBJ\x08\0\0\0Subject\0\
        INAM\x08\0\0\0A title\0\
        data\x04\0\0\0\0\0\0\0";
    assert_eq!(bytes.len(), 92);
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize,
        bytes.len() - 8
    );
    let fields = assert_all_input_paths(
        bytes,
        Status::Partial,
        &[
            ("WAV", "Title", "A title"),
            ("WAV", "Format", "Microsoft PCM"),
            ("WAV", "Samples Per Second", "8000"),
            ("WAV", "Bits Per Sample", "16"),
            ("WAV", "Duration", "00:00:00"),
        ],
    );
    assert!(!fields.keys().any(|(_, name)| name == "ISBJ"));
    // A supported INFO identifier at the identical position remains Complete.
    assert_eq!(&bytes[48..52], b"ISBJ");
    let mut supported = bytes.to_vec();
    supported[48..52].copy_from_slice(b"ICMT");
    assert_all_input_paths(
        &supported,
        Status::Complete,
        &[("WAV", "Comments", "Subject"), ("WAV", "Title", "A title")],
    );
}

fn riff_chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend_from_slice(&u32::try_from(body.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(body);
    if body.len() % 2 != 0 {
        bytes.push(0);
    }
    bytes
}

fn pcm_wave_with_extra(extra: &[u8]) -> Vec<u8> {
    let mut body = b"WAVE".to_vec();
    body.extend(riff_chunk(
        b"fmt ",
        b"\x01\0\x01\0\x40\x1f\0\0\x80\x3e\0\0\x02\0\x10\0",
    ));
    body.extend_from_slice(extra);
    body.extend(riff_chunk(b"data", &[0; 4]));
    riff_chunk(b"RIFF", &body)
}

#[test]
fn wav_unknown_chunks_and_lists_are_partial_but_samples_and_padding_are_not() {
    let mut adtl = b"adtl".to_vec();
    adtl.extend(riff_chunk(b"labl", b"\x01\0\0\0Label\0"));
    for extra in [
        riff_chunk(b"bext", &[0; 602]),
        riff_chunk(b"iXML", b"<BWFXML/>\0"),
        riff_chunk(b"LIST", &adtl),
    ] {
        assert_all_input_paths(
            &pcm_wave_with_extra(&extra),
            Status::Partial,
            &[
                ("WAV", "Format", "Microsoft PCM"),
                ("WAV", "Bits Per Sample", "16"),
            ],
        );
    }
    let mut padding = riff_chunk(b"JUNK", &[1, 2, 3]);
    padding.extend(riff_chunk(b"PAD ", &[4, 5]));
    assert_all_input_paths(
        &pcm_wave_with_extra(&padding),
        Status::Complete,
        &[
            ("WAV", "Duration", "00:00:00"),
            ("WAV", "Samples Per Second", "8000"),
        ],
    );
}

fn avi_with_extra(header_extra: &[u8], root_extra: &[u8]) -> Vec<u8> {
    let mut header = [0; 56];
    header[..4].copy_from_slice(&33333u32.to_le_bytes());
    header[24..28].copy_from_slice(&u32::from(!header_extra.is_empty()).to_le_bytes());
    header[32..36].copy_from_slice(&24u32.to_le_bytes());
    header[36..40].copy_from_slice(&12u32.to_le_bytes());
    let mut hdrl = b"hdrl".to_vec();
    hdrl.extend(riff_chunk(b"avih", &header));
    hdrl.extend_from_slice(header_extra);
    let mut payload = b"AVI ".to_vec();
    payload.extend(riff_chunk(b"LIST", &hdrl));
    payload.extend_from_slice(root_extra);
    riff_chunk(b"RIFF", &payload)
}

#[test]
fn avi_stream_format_is_partial_but_movie_samples_and_padding_are_not() {
    let mut stream_header = [0; 56];
    stream_header[..4].copy_from_slice(b"vids");
    stream_header[4..8].copy_from_slice(b"DIB ");
    stream_header[20..24].copy_from_slice(&1u32.to_le_bytes());
    stream_header[24..28].copy_from_slice(&30u32.to_le_bytes());
    let mut bitmap = [0; 40];
    bitmap[..4].copy_from_slice(&40u32.to_le_bytes());
    bitmap[4..8].copy_from_slice(&24u32.to_le_bytes());
    bitmap[8..12].copy_from_slice(&12u32.to_le_bytes());
    bitmap[12..14].copy_from_slice(&1u16.to_le_bytes());
    bitmap[14..16].copy_from_slice(&24u16.to_le_bytes());
    bitmap[20..24].copy_from_slice(&864u32.to_le_bytes());
    let mut strl = b"strl".to_vec();
    strl.extend(riff_chunk(b"strh", &stream_header));
    strl.extend(riff_chunk(b"strf", &bitmap));
    assert_all_input_paths(
        &avi_with_extra(&riff_chunk(b"LIST", &strl), &riff_chunk(b"LIST", b"movi")),
        Status::Partial,
        &[
            ("AVI", "Width", "24 pixels"),
            ("AVI", "Frames Per Second", "30"),
        ],
    );
    let mut payload = riff_chunk(b"LIST", b"movi");
    payload.extend(riff_chunk(b"JUNK", &[0; 3]));
    payload.extend(riff_chunk(b"PAD ", &[0; 2]));
    assert_all_input_paths(
        &avi_with_extra(&[], &payload),
        Status::Complete,
        &[
            ("AVI", "Width", "24 pixels"),
            ("AVI", "Height", "12 pixels"),
        ],
    );
}

#[test]
fn webp_unknown_chunk_is_partial_but_coded_alpha_and_padding_are_not() {
    // VP8X dimensions/flags are metadata; ALPH bytes are coded pixel payload.
    let mut base = b"WEBP".to_vec();
    base.extend(riff_chunk(b"VP8X", &[16, 0, 0, 0, 23, 0, 0, 11, 0, 0]));
    let mut with_unknown = base.clone();
    with_unknown.extend(riff_chunk(b"META", b"not decoded"));
    assert_all_input_paths(
        &riff_chunk(b"RIFF", &with_unknown),
        Status::Partial,
        &[("WebP", "Image Width", "24"), ("WebP", "Has Alpha", "true")],
    );
    base.extend(riff_chunk(b"ALPH", &[0; 1 + 24 * 12]));
    base.extend(riff_chunk(b"JUNK", &[0; 3]));
    assert_all_input_paths(
        &riff_chunk(b"RIFF", &base),
        Status::Complete,
        &[("WebP", "Image Width", "24"), ("WebP", "Has Alpha", "true")],
    );
}

#[test]
fn avi_unsupported_info_retains_main_header_fields() {
    // Complete 56-byte AVIMAINHEADER plus a supported RIFF INFO container whose
    // title interpretation is not implemented by the AVI metadata visitor.
    let mut header = [0; 56];
    header[..4].copy_from_slice(&33333u32.to_le_bytes());
    header[16..20].copy_from_slice(&1u32.to_le_bytes());
    header[24..28].copy_from_slice(&1u32.to_le_bytes());
    header[32..36].copy_from_slice(&24u32.to_le_bytes());
    header[36..40].copy_from_slice(&12u32.to_le_bytes());
    let mut hdrl = b"hdrl".to_vec();
    hdrl.extend(riff_chunk(b"avih", &header));
    let mut info = b"INFO".to_vec();
    info.extend(riff_chunk(b"INAM", b"A title\0"));
    let mut payload = b"AVI ".to_vec();
    payload.extend(riff_chunk(b"LIST", &hdrl));
    payload.extend(riff_chunk(b"LIST", &info));
    let bytes = riff_chunk(b"RIFF", &payload);
    assert_all_input_paths(
        &bytes,
        Status::Partial,
        &[
            ("AVI", "Width", "24 pixels"),
            ("AVI", "Height", "12 pixels"),
        ],
    );
}

fn jpeg_with_app0(body: &[u8]) -> Vec<u8> {
    // Independent baseline SOF: 24x12, three components, no sample decoding.
    let mut bytes = vec![0xff, 0xd8, 0xff, 0xe0];
    bytes.extend_from_slice(&u16::try_from(body.len() + 2).unwrap().to_be_bytes());
    bytes.extend_from_slice(body);
    bytes.extend_from_slice(&[
        0xff, 0xc0, 0, 17, 8, 0, 12, 0, 24, 3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1, 0xff, 0xd9,
    ]);
    bytes
}

#[test]
fn jfxx_thumbnail_is_partial_while_jfif_remains_supported() {
    // JFXX extension 0x13: one by one RGB thumbnail with three pixel bytes.
    let jfxx = jpeg_with_app0(b"JFXX\0\x13\x01\x01\x20\x40\x60");
    assert_all_input_paths(
        &jfxx,
        Status::Partial,
        &[
            ("JPEG", "Image Width", "24 pixels"),
            ("JPEG", "Image Height", "12 pixels"),
        ],
    );
    // JFIF 1.02, unitless one-dot density and no embedded thumbnail.
    let jfif = jpeg_with_app0(b"JFIF\0\x01\x02\0\0\x01\0\x01\0\0");
    assert_all_input_paths(
        &jfif,
        Status::Complete,
        &[
            ("JFIF", "Version", "1.2"),
            ("JFIF", "X Resolution", "1 dot"),
            ("JFIF", "Thumbnail Width Pixels", "0"),
            ("JPEG", "Image Width", "24 pixels"),
        ],
    );
}
