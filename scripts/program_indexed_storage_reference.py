#!/usr/bin/env python3
"""Emit a pinned real-VM reference host with bounded indexed-storage observations.

Only reference guard/provenance code is adapted. NIL edition bytes, the primary
parser/VM, every previous oracle source and the result protocol remain unchanged.
The sibling observer is embedded verbatim, so no additional include path is needed.
"""
from pathlib import Path
import argparse
import hashlib
import importlib.util
import json

hygiene_adapter_sha256 = "e0894c957a14eebab0290ac882d5ca9f904d051529891a812ff25b2ff20a80a5"
hygiene_source_sha256 = "fa9cc3cdc47eae81ead41b2475974d0bd2df3c78a5980728de5ba04c53976aa4"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def record(path, data):
    return {"path": str(path), "bytes": len(data), "sha256": digest(data)}


def adapt(source, observer):
    """Adapt exact generated profile-13 bytes; return bytes and ordered edit records."""
    if digest(source) != hygiene_source_sha256:
        raise ValueError("profile-13 generated reference identity changed")
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

    replace("body_shapes", "    case Opcode::NEW_ARRAY_LITERAL:\n        return args.size() == 2", "    case Opcode::ALLOC:\n        return args.size() == 1 && args[0] == \"i64\";\n    case Opcode::FREE:\n    case Opcode::SET_INDEX:\n        return args.empty();\n    case Opcode::NEW_ARRAY_LITERAL:\n        return args.size() == 2")
    replace("fallback_effect_guard", "                    declared == arities.end() || declared->second != *count)) {", "                    declared != arities.end() ? declared->second != *count :\n                    args[0] != \"array__push\" || *count != 2)) {")
    replace("observer", "class callable_provenance {", observer.decode() + "\nclass callable_provenance {")
    replace("observer_constructor", "    explicit callable_provenance(const std::vector<function_range> &functions)\n        : functions_(functions) {}", "    callable_provenance(const std::vector<function_range> &functions,\n                        indexed_storage_observer &storage)\n        : functions_(functions), storage_(storage) {}")
    replace("observe_before_tags", "        dispatched_target_.reset();\n        const auto &actual = running.getStack();", "        dispatched_target_.reset();\n        storage_.observed(running);\n        const auto &actual = running.getStack();")
    replace("local_callable_tag_domain", "                if (local == frames_.back().locals.end() || !local->second) {", "                if (local == frames_.back().locals.end() || !local->second ||\n                    *local->second >= functions_.size()) {")
    replace("fallback_dispatch", "            } else if (!console_builtin(args[0])) {\n                target = find_function(args[0]);\n            }", "            } else if (!console_builtin(args[0])) {\n                for (std::size_t index = 0; index < functions_.size(); ++index) {\n                    if (functions_[index].name == args[0]) {\n                        target = index;\n                        break;\n                    }\n                }\n                if (!target && args[0] != \"array__push\") {\n                    refuse(\"unknown internal target\");\n                }\n            }")
    replace("fallback_effect", "            stack_.resize(stack_.size() - count);\n            return true;", "            if (args[0] == \"array__push\") {\n                storage_.append(actual);\n            }\n            stack_.resize(stack_.size() - count);\n            return console_builtin(args[0]);")
    replace("storage_provenance", "        case Opcode::NEW_ARRAY_LITERAL:\n            consume(*array_literal_count(args[0]), true);\n            break;", """        case Opcode::NEW_ARRAY_LITERAL:
            require_noncallable(*array_literal_count(args[0]));
            storage_.literal(actual, *array_literal_count(args[0]), args[1] == "text");
            consume(*array_literal_count(args[0]), true);
            break;
        case Opcode::ALLOC: {
            require_noncallable(1);
            const auto identity = storage_.allocate(actual);
            stack_.back() = functions_.size() + identity;
            break;
        }
        case Opcode::FREE:
            require_values(1);
            storage_.release(actual, buffer_tag(stack_.size() - 1));
            stack_.pop_back();
            break;
        case Opcode::GET_INDEX:
            require_values(2);
            require_noncallable(1);
            storage_.get(actual, buffer_tag(stack_.size() - 2));
            stack_.resize(stack_.size() - 2);
            stack_.push_back(std::nullopt);
            break;
        case Opcode::SET_INDEX:
            require_values(3);
            require_noncallable(2);
            storage_.set(actual, buffer_tag(stack_.size() - 3));
            stack_.resize(stack_.size() - 3);
            break;
        case Opcode::LEN:
            require_noncallable(1);
            storage_.length(actual);
            consume(1, true);
            break;""")
    # Change only this monitor's generic cases, preserving inherited record,
    # enum and floating guards for their own kinds.
    begin = source.index(b"class callable_provenance {")
    end = source.index(b"class record_provenance {", begin)
    fragment = source[begin:end].decode()
    changed = fragment.replace("case Opcode::CMP_GE: case Opcode::STR_CONCAT: case Opcode::GET_INDEX:", "case Opcode::CMP_GE: case Opcode::STR_CONCAT:")
    changed = changed.replace("case Opcode::CAST: case Opcode::NEG: case Opcode::NOT: case Opcode::LEN:", "case Opcode::CAST: case Opcode::NEG: case Opcode::NOT:")
    replace("indexed_consumption_split", fragment, changed)
    replace("origin_tag_validation", "    void check_tag(const ny::Value &value, tag provenance) const {\n        if (provenance) {", """    std::optional<std::size_t> buffer_tag(std::size_t index) const {
        const auto provenance = stack_[index];
        return provenance && *provenance >= functions_.size()
            ? std::optional<std::size_t>(*provenance - functions_.size()) : std::nullopt;
    }
    void check_tag(const ny::Value &value, tag provenance) const {
        storage_.check_array(value);
        if (provenance && *provenance >= functions_.size()) {
            storage_.require_buffer(value, *provenance - functions_.size());
            return;
        }
        if (provenance) {""")
    replace("origin_tag_member", "    std::vector<tag> stack_;", "    indexed_storage_observer &storage_;\n    std::vector<tag> stack_;")
    replace("main_observer", "        callable_provenance provenance(functions);", "        indexed_storage_observer storage;\n        callable_provenance provenance(functions, storage);")
    replace("remove_old_receiver_guard", """            } else if (instruction.op == ny::Opcode::GET_INDEX) {
                if (stack.size() < 2 ||
                    !std::holds_alternative<ny::ArrayHandle>(stack[stack.size() - 2]) ||
                    !std::holds_alternative<std::int64_t>(stack.back())) {
                    throw std::runtime_error("reference array probe: GET_INDEX requires array handle and integer");
                }
""", "")
    replace("final_observation", "        records.observed(vm);\n        floats.observed(vm);", "        records.observed(vm);\n        floats.observed(vm);\n        storage.observed(vm);")
    return source, changes


def generate():
    directory = Path(__file__).resolve().parent
    previous_path = directory / "program_hygienic_locals_reference.py"
    previous_bytes = previous_path.read_bytes()
    if digest(previous_bytes) != hygiene_adapter_sha256:
        raise ValueError("profile-13 reference adapter identity changed")
    spec = importlib.util.spec_from_file_location("pinned_hygienic_reference", previous_path)
    previous = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(previous)
    base_path = directory / "program_nested_records_reference.cpp"
    base_bytes = base_path.read_bytes()
    inherited, insertion_offset = previous.adapt(base_bytes)
    observer_path = directory / "program_indexed_storage_observer.hpp"
    observer_bytes = observer_path.read_bytes()
    generated, changes = adapt(inherited, observer_bytes)
    return generated, {
        "base_source": record(base_path, base_bytes),
        "hygiene_adapter_source": record(previous_path, previous_bytes),
        "hygiene_generated_source": {"bytes": len(inherited), "sha256": digest(inherited),
                                     "insertion_offset": insertion_offset},
        "observer_source": record(observer_path, observer_bytes),
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
        "schema": 1, "strategy": "exact_pinned_source_with_indexed_storage_guard_and_origin_observer",
        **provenance, "adapter_source": record(adapter, adapter.read_bytes()),
        "generated_source": record(output, generated),
        "edition_bytes_rewritten": False, "prior_source_modified": False,
        "reference_execution": "Pinned parseProgram/parseTextNilModule and VM; no element-value interpreter",
        "reference_limits": {"descriptors": 1024, "charged_cells": 16384,
                             "append_charge": "one cell; no candidate relocation simulation"},
        "repeated_free": "guard refusal; actual VM stderr diagnostic excluded from quiet protocol",
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
