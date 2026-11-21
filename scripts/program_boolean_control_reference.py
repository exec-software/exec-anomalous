#!/usr/bin/env python3
"""Emit a narrowly extended actual-VM oracle for boolean control and f64 console.

The pinned VM already represents booleans as integer 0/1 and executes their
branches/NOT. Only the reference's floating console CALL restriction changes.
Raw console opcodes and intrinsic interop retain their previous f64 refusal;
notably intrinsic console uses std::to_string rather than classic ostream.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json

parent_adapter_sha256 = "cb1cc9abb375d181ce06d71548358956eec5f51a394131c637dfbe3bc9783e0f"
parent_observer_sha256 = "adac8143ce68f475ca33d6df9e4495595a2b5788436b07dd2ab5f5d8f7e357ec"
parent_generated_sha256 = "e4ed17d0da4c267eec3d861c6ed164df3e0379f7f46b14c7a8a461d03f53dc62"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def record(path, data):
    return {"path": str(path), "bytes": len(data), "sha256": digest(data)}


def adapt(source):
    if digest(source) != parent_generated_sha256:
        raise ValueError("profile-14 generated reference identity changed")
    changes = []
    def replace(label, old, new):
        nonlocal source
        old, new = old.encode(), new.encode()
        if source.count(old) != 1:
            raise ValueError("non-unique reference anchor: " + label)
        offset = source.index(old)
        changes.append({"name": label, "offset": offset, "before_bytes": len(old),
                        "before_sha256": digest(old), "after_bytes": len(new),
                        "after_sha256": digest(new)})
        source = source[:offset] + new + source[offset + len(old):]
    replace("floating_environment_include", "#include <charconv>\n", "#include <charconv>\n#include <cfenv>\n")
    replace("classic_console_guard", "// Inspect actual VM values, never evaluate an instruction in this observer.\n", """// Profile 15 retains the actual VM's ostream insertion, including rounding.
// Refuse an unexpected host format/environment rather than formatting ourselves
// or changing the VM's bytes. Only the redirected normal output is inspected.
void guard_classic_console(const std::ostream &output) {
    const auto flags = output.flags();
    if (output.precision() != 6 || output.width() != 0 ||
        (flags & std::ios::floatfield) != std::ios::fmtflags(0) ||
        (flags & (std::ios::showpoint | std::ios::showpos | std::ios::uppercase)) !=
            std::ios::fmtflags(0) || output.getloc() != std::locale::classic() ||
        std::fegetround() != FE_TONEAREST) {
        throw std::runtime_error("reference boolean-control probe: classic console environment drift");
    }
}

// Inspect actual VM values, never evaluate an instruction in this observer.
""")
    replace("finite_console_call", """            } else if (console_call) {
                reject_floating(stack, 1);
            }
""", """            } else if (console_call) {
                if (stack.empty()) {
                    refuse("floating operand underflow");
                }
                // Real CALL console builtins stream finite doubles directly.
                // Raw opcodes/INTEROP below retain their prior refusal policy.
                require_finite(stack.back());
            }
""")
    replace("initial_console_guard", "    try {\n        std::istringstream input(edition);", "    try {\n        guard_classic_console(output);\n        std::istringstream input(edition);")
    replace("console_callback_capture", "&enumerations, &globals, &floats](ny::VM &running) {", "&enumerations, &globals, &floats, &output](ny::VM &running) {")
    replace("console_boundary_guard", "            if (steps == maximum_steps) {", "            guard_classic_console(output);\n            if (steps == maximum_steps) {")
    replace("final_console_guard", "        storage.observed(vm);", "        storage.observed(vm);\n        guard_classic_console(output);")
    return source, changes


def generate():
    directory = Path(__file__).resolve().parent
    parent_path = directory / "program_indexed_storage_reference.py"
    parent_bytes = parent_path.read_bytes()
    observer_path = directory / "program_indexed_storage_observer.hpp"
    if digest(parent_bytes) != parent_adapter_sha256:
        raise ValueError("profile-14 reference adapter identity changed")
    if digest(observer_path.read_bytes()) != parent_observer_sha256:
        raise ValueError("profile-14 reference observer identity changed")
    spec = importlib.util.spec_from_file_location("pinned_indexed_reference", parent_path)
    parent = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(parent)
    inherited, inherited_provenance = parent.generate()
    generated, changes = adapt(inherited)
    return generated, {
        "parent_adapter_source": record(parent_path, parent_bytes),
        "parent_generated_source": {"bytes": len(inherited), "sha256": digest(inherited)},
        "parent_provenance": inherited_provenance,
        "changes": changes,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    generated, provenance = generate()
    output = args.output.resolve()
    receipt_path = output.with_suffix(".json")
    if output == receipt_path or output.exists() or receipt_path.exists():
        parser.error("generated source and receipt must be distinct new files")
    adapter = Path(__file__).resolve()
    receipt = {
        "schema": 1, "strategy": "exact_pinned_source_with_finite_console_call_guard_extension",
        **provenance, "adapter_source": record(adapter, adapter.read_bytes()),
        "generated_source": record(output, generated),
        "edition_bytes_rewritten": False, "prior_source_modified": False,
        "reference_execution": "Pinned parser/VM; actual ostream insertion; no replacement formatter",
        "format_environment": {"locale": "classic", "floatfield": "defaultfloat", "precision": 6,
                               "rounding": "FE_TONEAREST", "positive_sign": False, "uppercase": False},
        "floating_console_scope": "Only inherited console CALL builtins; raw opcodes and intrinsic interop stay guarded",
    }
    encoded = (json.dumps(receipt, indent=2, sort_keys=True) + "\n").encode()
    with output.open("xb") as stream:
        stream.write(generated)
    with receipt_path.open("xb") as stream:
        stream.write(encoded)
    print(json.dumps({"source": receipt["generated_source"],
                      "receipt": record(receipt_path, encoded)}, sort_keys=True))


if __name__ == "__main__":
    main()
