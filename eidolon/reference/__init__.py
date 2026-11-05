"""Dependency-free reference implementation of the Eidolon draft format."""

from .codec import (
    default_limits,
    eidolon_error,
    decoded_frame,
    decoded_trace,
    resource_limits,
    crc32c,
    decode_trace,
    encode_trace,
    read_ppm,
    write_ppm,
)

__all__ = [
    "default_limits",
    "eidolon_error",
    "decoded_frame",
    "decoded_trace",
    "resource_limits",
    "crc32c",
    "decode_trace",
    "encode_trace",
    "read_ppm",
    "write_ppm",
]
