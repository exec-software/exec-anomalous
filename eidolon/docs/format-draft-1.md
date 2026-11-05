# Eidolon lossless framebuffer trace: experimental draft 1

## Status

This document describes the byte-exact format implemented by the Python
reference oracle. It is an experimental conformance target, not an accepted or
stable Anomalous wire format. Incompatible revisions may replace it while
[ADR 0017](../../docs/adr/0017-eidolon-lossless-framebuffer-traces.md) remains
Proposed.

- Conventional extension: `.eidolon`
- Proposed media type: `application/vnd.anomalous.eidolon`
- Byte order: little-endian
- Pixel order: packed RGB, eight bits per channel, row-major, top to bottom

All reserved flag bits are zero. Draft-1 readers reject rather than ignore an
unknown flag, mode, header size, pixel format, or trailing byte.

## Integrity primitive

Every CRC field is CRC-32C (Castagnoli) with reflected polynomial
`0x82f63b78`, initial value `0xffffffff`, and final XOR `0xffffffff`. The check
value for ASCII `123456789` is `0xe3069283`.

CRC32C detects accidental corruption. It is not a signature, authentication
code, provenance record, or safety boundary against a malicious producer.

## File header

The file begins with one fixed 48-byte header. The header CRC covers bytes 0
through 43.

| Offset | Size | Field | Draft-1 requirement |
| ---: | ---: | --- | --- |
| 0 | 8 | `magic` | bytes `EIDOLON\0` |
| 8 | 2 | `version` | unsigned integer `1` |
| 10 | 2 | `header_size` | unsigned integer `48` |
| 12 | 4 | `flags` | zero |
| 16 | 4 | `width` | nonzero pixels |
| 20 | 4 | `height` | nonzero pixels |
| 24 | 4 | `pixel_format` | `1` for packed RGB8 |
| 28 | 2 | `tile_width` | nonzero pixels |
| 30 | 2 | `tile_height` | nonzero pixels |
| 32 | 4 | `timebase_numerator` | nonzero seconds numerator |
| 36 | 4 | `timebase_denominator` | nonzero seconds denominator |
| 40 | 4 | `frame_count` | nonzero number of following frames |
| 44 | 4 | `header_crc32c` | CRC32C of bytes 0..43 |

A timestamp tick represents
`timebase_numerator / timebase_denominator` seconds. Geometry applies to every
frame in the file. Draft 1 has no colorspace profile, alpha channel, dynamic
geometry change, audio, metadata, or random-access index.

## Frame record

Exactly `frame_count` frame records follow with no padding. Each begins with a
fixed 44-byte header. Its header CRC covers bytes 0 through 39 of that header.

| Offset | Size | Field | Draft-1 requirement |
| ---: | ---: | --- | --- |
| 0 | 4 | `magic` | bytes `FRM1` |
| 4 | 2 | `header_size` | unsigned integer `44` |
| 6 | 2 | `flags` | bit 0 is `key_frame`; every other bit is zero |
| 8 | 4 | `frame_index` | zero-based, contiguous, in file order |
| 12 | 8 | `timestamp_ticks` | monotonically nondecreasing |
| 20 | 4 | `duration_ticks` | nonzero |
| 24 | 4 | `uncompressed_size` | exactly `width * height * 3` |
| 28 | 4 | `payload_size` | bytes following this header |
| 32 | 4 | `payload_crc32c` | CRC32C of the encoded payload |
| 36 | 4 | `decoded_crc32c` | CRC32C of the complete decoded RGB frame |
| 40 | 4 | `header_crc32c` | CRC32C of header bytes 0..39 |

Frame zero must be a key frame. A key frame has no temporal reference. A delta
frame refers only to the complete preceding decoded frame. A decoder publishes
no frame until its tile payload and complete decoded-frame CRC both pass.

## Tile traversal and records

Tiles cover the frame in raster order, left to right and then top to bottom.
The final tile in either dimension is clipped to the frame edge. The tile count
is therefore:

```text
ceil(width / tile_width) * ceil(height / tile_height)
```

Each frame payload contains exactly that many tile records and nothing else.
A tile record begins with:

| Offset | Size | Field |
| ---: | ---: | --- |
| 0 | 1 | `mode` |
| 1 | 4 | `tile_payload_size` |
| 5 | variable | mode payload |

Draft-1 modes are:

| Mode | Name | Payload and validation |
| ---: | --- | --- |
| 0 | `raw` | Exactly the packed RGB bytes for this tile |
| 1 | `solid` | Exactly one three-byte RGB pixel, repeated for the tile |
| 2 | `pixel_rle` | Repeated little-endian `u16 run`, three-byte RGB pixel pairs |
| 3 | `unchanged` | Empty; copy the prior frame's corresponding tile |
| 4 | `xor_rle` | Repeated little-endian `u16 run`, one-byte value pairs; XOR the decoded byte stream with the prior tile |

Every run is nonzero and must fill the tile exactly without exceeding it.
Temporal modes 3 and 4 are forbidden in key frames. An empty, short, long, or
otherwise noncanonical decoded tile is rejected.

## Deterministic encoding

The reference encoder considers `raw`, `solid`, and `pixel_rle` for every tile.
For delta frames it also considers `unchanged` and `xor_rle`. It selects the
smallest mode payload. Equal sizes use this priority:

```text
unchanged, solid, pixel_rle, xor_rle, raw
```

RLE consumes the longest possible run up to 65,535 values before beginning the
next run. A key frame is emitted for frame zero and every configured keyframe
interval. These rules make identical inputs and encoder options byte-identical.

## Decoder budgets and failure behavior

Wire integers alone are never allocation authority. Before multiplication,
allocation, or slicing, a decoder checks geometry, tile count, decoded frame
size, frame count, payload size, and total output against local policy.

The reference oracle defaults to width and height at most 16,384, a tile edge
at most 256, 10,000 frames, 1,048,576 tiles per frame, 256 MiB decoded per
frame, 512 MiB encoded per frame, a 1 GiB input container, and 512 MiB total
decoded output. Those are development-tool budgets, not a promise that a
future vessel will accept files of those sizes. Callers may reduce them.

Any violation fails the complete decode with a structured exception. The
reference API currently materializes the whole checked trace; a future
streaming decoder must preserve the same never-publish-before-verification rule
while applying an explicit retained-frame budget.

Ny's current interpreted implementation applies a stricter local profile of
1 MiB each for a frame, encoded payload, input container, and total decoded
output. This is not a draft-1 wire restriction or an RSS guarantee; it is an
empirical gate around the current generic-value VM representation.

## P6 input profile

The host encoders accept the binary P6 PPM subset emitted by QEMU
`screendump`. The file begins with the magic `P6` at byte zero; leading
whitespace or a leading comment is rejected. Width and height are unsigned
ASCII decimal tokens, `maxval` is exactly `255`, and the raster contains exactly
`width * height * 3` bytes. ASCII whitespace and `#` line comments are accepted
between header tokens. The complete header, including the one-byte whitespace
separator or complete CRLF pair immediately before the raster, is at most 4096
bytes. An LF may therefore end at byte offset 4096; a CRLF that would extend to
offset 4097 is rejected.
