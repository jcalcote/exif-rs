# Source adaptation and synthetic reference data

Selected reader structures, tag names and human-readable descriptions are adapted
from metadata-extractor 2.19.0, Copyright 2002-2019 Drew Noakes and contributors,
Apache-2.0. Upstream: https://github.com/drewnoakes/metadata-extractor/tree/2.19.0

`LICENSE-APACHE-2.0` preserves the upstream license verbatim. The pinned upstream
root/source archives contained no companion NOTICE. Adapted Rust file headers
identify source-derived behavior; `upstream-sources.json` records exact source
class and validation artifact hashes. The implementation changes language,
allocation, bounds, cancellation and error handling, and is an incomplete port.

All input fixtures here are independently constructed public synthetic bytes,
with no photographs, private media or application payloads. Reference fields were
recorded using metadata-extractor 2.19.0 and xmpcore 6.1.11, US locale and UTC. They
are regression evidence for descriptions, not a public API specification or a
claim of exhaustive compatibility. No Java executable, wrapper, JAR or classfile
is included or required to run the Rust tests. Reference field prefixes and error
sentinels were removed; the synthetic 65-byte text case now verifies retention.

Known deliberate differences: a tiny WAV final chunk has its correct zero-second
duration; malformed payloads and duplicate/missing ICC fragments fail explicitly
rather than retaining recovered fields. Reader/descriptor/format limitations are
reported with explicit coverage status. Production extraction does not perform
file I/O, decode pixels, spawn subprocesses, or install a global allocator.

Ordinary TIFF headers with IFD offset 8 are classified as TIFF. The upstream
reference classified this header as Sony ARW without camera-specific evidence;
comparison tests assert the intentional TIFF result without altering those
reference field values. Distinct CR2/ORF/RW2 signatures retain their own names.
PDF headers are recognition-only and return Partial coverage.

JPEG ICC sequence descriptors occupy 4,080 bytes (255 x 16) within the unchanged
128-KiB scratch arena. Each descriptor stores a checked 64-bit source offset and
length, avoiding the four-byte source-offset restriction. Borrowed total input
length has no application prefix cap; at most 4,096 segment descriptors are
accepted and actual parsing remains constrained by the shared work limit.

The three-component synthetic JPEG SOF also verifies descriptive component fields
that the historical length-filtered reference omitted. Tests assert the literal
SOF IDs, sampling nibbles and quantization selectors and the three exact extra
component descriptions, then compare every remaining field to unchanged reference
rows. No general filtering of unexpected fields is performed.
