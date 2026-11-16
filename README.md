# exif-rs

Metadata extraction for images and selected media containers, written in Rust.
The crate is **`exif-inspect`**; the repository is `exif-rs` because that crate name
is already in use.

The library reads immutable byte slices, including segmented input, and returns
structured textual fields with separate directory, name and value components.
There are no application-specific flattened keys, error sentinels, executors,
filesystem discovery or service dependencies.

## Use

The initial version is available from Git; it has not been published to crates.io:

```toml
[dependencies]
exif-inspect = { git = "https://github.com/jcalcote/exif-rs", tag = "v0.1.0" }
```

```rust,no_run
let bytes = std::fs::read("photo.jpg")?;
let metadata = exif_inspect::extract(&bytes)?;
for field in metadata.fields() {
    println!("{} / {} = {}", field.directory, field.name, field.value);
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

The convenience function allocates scratch and detached output. The low-level
`Extractor` API accepts caller-owned scratch/output and segmented borrowed input,
with explicit planning, resource diagnostics and cooperative cancellation.
Parsing and iteration on that path allocate no heap. Results borrow only output
storage, so callers can release source and scratch after extraction returns.

Always inspect extraction status. A supported header does not imply that every
metadata section in that file has been decoded. Unsupported encountered sections
report partial coverage; malformed input and exhausted resource limits are distinct.
No result establishes that a file is safe to open in another application.

## Initial coverage

Implemented readers cover selected fields in TIFF/Exif/GPS, JPEG/JFIF, PNG headers
and plain text, BMP, GIF headers, ICO, PCX, WebP, WAV, AVI, MP3, common TIFF-based RAW
headers, RAF, PSD, EPS and selected MOV/MP4/HEIF boxes. Selected IPTC and ICC fields
are supported through JPEG, TIFF and PSD carriers, including validated split JPEG
ICC profiles.

This is **not complete coverage of those formats**. Maker notes, XMP, compressed
metadata, many media-track/item descriptions and additional ICC/IPTC types remain
future work. The first API exposes formatted textual descriptions, not a lossless
raw-value model. Repeated directory/name pairs currently retain the last value;
separate directory occurrences are not represented. These limits are explicit
rather than inferred raw types or promises of exhaustive extraction.

## Limits and ownership

Input is borrowed and may exceed 4 MiB; its combined length is checked and at most
4,096 source segments are accepted. The initial output limits are 512 retained
fields and 256 KiB of packed output. Scratch is 128 KiB, nesting is capped at 32,
and parse/lookup/formatting work is limited to 1,000,000 operations. Each text
component is limited to 1,024 UTF-8 bytes; overflow is a resource-limit outcome.
The library never silently reads a file prefix on a caller's behalf. Applications
select their own input and map diagnostics according to their requirements.

Caller-provided storage is checked before use. A detached output descriptor can be
retained beside an owned output buffer and validates offsets, lengths and UTF-8
when reborrowed. It is not a cryptographic proof against caller modification.

Minimum supported Rust: **1.85**, edition 2024. No runtime dependencies or JVM.
Tests use public synthetic fixtures, format/description references, malformed data,
segmented reads, output corruption, cancellation and resource boundaries. CI checks
Linux, macOS, Windows, minimum Rust, documentation and distributable packaging.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## License and attribution

Apache-2.0; see [LICENSE](LICENSE) and [NOTICE](NOTICE). Selected readers, lookup
tables and descriptions are adapted from
[metadata-extractor 2.19.0](https://github.com/drewnoakes/metadata-extractor/tree/2.19.0),
with upstream notices and source hashes retained. The Rust implementation changes
the allocation model, traversal, validation and supported coverage.
