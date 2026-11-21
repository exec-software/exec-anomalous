#!/usr/bin/env python3
"""Emit one reviewed local-name guard extension over the pinned profile-12 VM oracle.

This is a build-input adapter, never an edition adapter or interpreter. The
existing reference source is immutable. Its real NIL parser, VM, observations,
resource guards and result protocol are retained byte-for-byte around one
inserted predicate branch. Compile the emitted C++ with the same pinned NIL
archives and include directory as program_nested_records_reference.cpp.
"""
from pathlib import Path
import argparse
import hashlib
import json


base_sha256 = "59a1bf0b4272a65bca19cfef4be5074810bfbea785b3e723bd3a874b6142ebbc"
anchor = b"bool local_name(const std::string &name) {\n"
insertion = b'''    // Profile 13: generated storage is an exact frame-local string identity.
    // The counter is canonical decimal spelling, not a parsed numeric index.
    constexpr const char *hygienic_prefix = "$ny.local.";
    constexpr std::size_t hygienic_prefix_size = 10;
    if (name.compare(0, hygienic_prefix_size, hygienic_prefix) == 0) {
        if (name.size() > 128) {
            return false;
        }
        const auto separator = name.find('.', hygienic_prefix_size);
        if (separator == std::string::npos || separator == hygienic_prefix_size) {
            return false;
        }
        const auto counter = name.substr(hygienic_prefix_size,
                                         separator - hygienic_prefix_size);
        if (counter.size() > 1 && counter.front() == '0') {
            return false;
        }
        for (const unsigned char byte : counter) {
            if (byte < '0' || byte > '9') {
                return false;
            }
        }
        return safe_name(name.substr(separator + 1));
    }
'''


def digest(data):
    return hashlib.sha256(data).hexdigest()


def record(path, data):
    return {"path": str(path), "bytes": len(data), "sha256": digest(data)}


def adapt(source):
    if digest(source) != base_sha256:
        raise ValueError("profile-12 reference source identity changed")
    if source.count(anchor) != 1:
        raise ValueError("local-name insertion anchor is not unique")
    offset = source.index(anchor) + len(anchor)
    generated = source[:offset] + insertion + source[offset:]
    return generated, offset


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True,
                        help="new external C++ build input; sibling .json records provenance")
    args = parser.parse_args()
    adapter = Path(__file__).resolve()
    source_path = adapter.with_name("program_nested_records_reference.cpp")
    source = source_path.read_bytes()
    generated, offset = adapt(source)
    output = args.output.resolve()
    receipt_path = output.with_suffix(".json")
    if output == receipt_path or output.exists() or receipt_path.exists():
        parser.error("generated source and receipt must be distinct new files")
    receipt = {
        "schema": 1,
        "strategy": "exact_pinned_source_with_one_local_name_guard_insertion",
        "base_source": record(source_path, source),
        "adapter_source": record(adapter, adapter.read_bytes()),
        "generated_source": record(output, generated),
        "insertion": {"offset": offset, "bytes": len(insertion),
                      "sha256": digest(insertion), "anchor": anchor.decode()},
        "edition_bytes_rewritten": False,
        "prior_source_modified": False,
        "reference_execution": "Pinned parseProgram/parseTextNilModule and VM; inherited observers and caps",
    }
    encoded_receipt = (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()
    # Exclusive creation preserves earlier build evidence on a repeated command.
    with output.open("xb") as stream:
        stream.write(generated)
    with receipt_path.open("xb") as stream:
        stream.write(encoded_receipt)
    print(json.dumps({"source": receipt["generated_source"],
                      "receipt": record(receipt_path, encoded_receipt)}, sort_keys=True))


if __name__ == "__main__":
    main()
