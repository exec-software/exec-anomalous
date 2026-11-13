// Independent hosted semantic oracle for bounded profile-9 nominal-enum editions.
// The qualified NIL checkout is read-only. From the Anomalous repository:
//
// nil_root=/path/to/qualified/nil-context-registration
// c++ -std=c++17 -O2 -I"$nil_root/nilcompiler/src" \
//     scripts/program_enum_reference.cpp \
//     "$nil_root/build/context-registration/lib/libnilcompiler_core.a" \
//     "$nil_root/build/context-registration/lib/libnil.a" \
//     -ldl -pthread -o build/program_enum_reference
//
// CLI: program_enum_reference EDITION_PATH
// One JSON record, with full signed-i64 result and byte-exact output_hex.
// Both semantic return and refusal exit 0. Usage/read failures exit 2 with a
// diagnostic on stderr. Reference exception text is preserved in error.
//
// This is a bounded test host, not a general NIL sandbox or an admission
// implementation. It calls the qualified VM rather than duplicating execution
// semantics. Whole-edition candidate admission is a separate operation. The
// effect guard accepts direct typed calls and function-local branches, including
// known instructions in dead blocks and the generated fallthrough RET marker.
// It permits only proven internal indirect calls; no imports, native calls,
// input, filesystem, environment, AI or UI effects. The only intrinsic forms
// are console output.
// The fresh VM has no FFI registrations for console's hot-reload hook to visit.
// Profiling is disabled before VM setup. Main is entered directly; generated
// module function-skip scaffolding is never dispatched.
//
// Static bounds: 64 KiB edition, 4096 body instructions, 64 functions, 128
// constants, 128 locals per function, 8 parameters. Main is zero-argument i32
// or i64; helpers may use i32/i64/text/array<i64>/array<text> and declared
// record/enum parameters, and those value kinds or void returns.
// Dynamic probe bounds: 1,000,000 VM instruction boundaries, 64 call frames
// including main, 4096 operand values, 1 MiB output. These deliberately exceed
// the candidate's defaults, allowing finite candidate resource-refusal cases
// to finish here. Oracle-limit refusals are NOT language equivalence evidence.
// Never submit infinite-loop cases as successful semantic comparisons.
//
// The VM callback counts main's generated entry LABEL once. A user CALL skips
// its generated entry LABEL; a jump dispatches the written destination LABEL.
// Consequently this callback budget is not the candidate's body-step counter.
// Text branch conditions retain the qualified VM semantics (both conditional
// branches fall through); integer-only conditions are candidate admission policy.
//
// Profile 6 inherits the profile-3 scalar subrestriction of qualified 4dad051.
// Generated locals are accepted as LOAD_LOCAL/STORE_LOCAL operands and as
// profile-6 local CALL targets, with spelling
// $ny.temp.<simple ASCII identifier>.<canonical decimal>, at most 128 bytes.
// Dotted compiler prefixes (for example query.source) remain outside this
// profile. Function, constant, label and direct CALL names remain ordinary
// identifiers; generated names cannot identify declared functions.
// Pinned NIL source: codegen.cpp:1220-1226 and 6114-6119 generate per-function
// scratch names; 2202-2246 and 2256-2325 lower range loops with operandless ADD.
// vm.cpp:290-368 selects i64 arithmetic for two integer values and wraps its
// result; 6766-6792 also supports floats/arrays in the general VM. This guard
// permits no float producer and rejects array/callable operands dynamically,
// so bare ADD retains exact scalar i64 semantics for integers and the VM's own
// refusal for text operands. No instruction is rewritten, and all prior
// reference sources remain independently reproducible.
//
// Profile 4 adds exact signed hexadecimal i64 literals and STR_CONCAT. The
// latter is restricted dynamically to two text operands; the general VM's
// numeric-to-text coercion is outside this candidate profile. Literal text is
// decoded once by nil_loader.cpp:1708 and concatenation copies decoded bytes
// in vm.cpp:7399. Frame return preserves the value (vm.cpp:2550). The reference
// host caps cumulative concatenated bytes at 1 MiB and live stack/local string
// bytes at 16 MiB. These are oracle safety limits, never semantic matches for a
// candidate resource refusal. Immutable literals remain part of the 64 KiB input.
//
// Profile 5 adds immutable homogeneous i64/text arrays, exact array<i64> and
// array<text> helper signatures, LEN and GET_INDEX. Pinned vm.cpp:7150 creates
// ArrayHandle values and preserves source element order; 1766 checks element
// types and retains full-width i64 values. At 7199, GET_INDEX also accepts raw
// integer pointer aliases in the general VM: this host explicitly requires the
// distinct ArrayHandle alternative instead. At 7291 LEN is similarly restricted
// to ArrayHandle. Default-edition out-of-range reads return typed 0/empty text
// (1530-1571, 7209-7217); the diagnostic strict-bounds override is disabled here.
// There are no mutation, free, nested-array or pointer producers. The host caps
// cumulative literals at 1024 arrays, 16384 elements and 16 MiB copied text,
// with at most 4096 elements per literal. These exceed candidate budgets and
// are independent oracle refusal limits, never matching candidate exhaustion.
//
// Profile 6 adds PUSH_FN_PTR and lexical local CALL. The pinned VM represents
// function references as strings (vm.cpp:6852), so an independent dynamic
// provenance monitor marks only PUSH_FN_PTR results and follows their moves
// through stack/local storage. It never computes arithmetic, text, branches,
// output or return values. Each callback checks its stack/frame shape and all
// tagged values against the actual VM. Ordinary text cannot forge a callable.
// Calls to known lexical local slots require an initialized tagged reference;
// the general VM's non-string fallback is outside this closed profile. The
// qualified module parser separately supplies original parameter names, which
// the executable parser does not retain. No callable parameters or returns,
// captured environment, callable arrays, builtin references or foreign targets
// are admitted. Direct and indirect calls share the original VM frame limits.
//
// Profile 7 adds declared nominal records with i64 fields. The qualified parser
// supplies schemas and per-declaration context; record context is not main intent.
// NEW_STRUCT creates an empty shared StructData (vm.cpp:6857), GET_FIELD returns
// zero for an unset field (6866), and SET_FIELD consumes only the value, retains
// the owner, and mutates aliases (6938). The general VM also permits undeclared
// fields and ordinary-record nominal mismatches; this host explicitly closes
// those cases. Only NEW_STRUCT can register a record identity. A shared_ptr
// ledger preserves actual VM identity until the probe ends, prevents allocator
// address reuse, and accounts cumulative allocation without computing fields.
// Each callback checks all stack/local record identities against that ledger.
// Calls and returns require exact declared nominal types. Scalar, array, text,
// callable and console operations cannot consume records. No casts, nested
// fields, classes, free/reclamation, pointer aliases or equality are enabled.
// Schemas are bounded at 64 layouts, 32 fields each and 256 declared fields in
// total. Dynamic oracle limits are 1024 objects and 16384 declared i64 cells;
// these exceed candidate limits and are never equivalence evidence for exhaustion.
//
// Profile 8 adds exactly LOAD_GLOBAL/STORE_GLOBAL NAME i64 and integer CAST
// text. Global names are ASCII identifier components joined by ::, at most 128
// bytes; names have no reserved-word restriction. The whole-file access set is
// bounded at 1024 slots (candidate 128), including dead accesses and load-only
// names. The actual VM owns the global map and every stored integer; the host
// checks names and types without calculating or mirroring global values.
// LOAD copies a stored i64 or preserves the VM's `Global not found: NAME`
// refusal (vm.cpp:1861). STORE consumes one i64 and overwrites (1873). Globals
// are cleared on VM reset (790); __globals_init__ is an ordinary helper run
// only by an explicit call, including the compiler's emitted main prologue
// (codegen.cpp:6574,6817). No default zero, implicit initialization or global
// fallback through LOAD_LOCAL/CALL is introduced. Existing callable provenance
// checks reject missing locals before getLocal's general global fallback.
// CAST text uses the actual VM's std::to_string(int64) at vm.cpp:7532. The general VM
// also accepts text/float inputs, which this host rejects. Decimal digit counts
// are used only for shared cumulative computed-text budget accounting; actual
// text is neither calculated nor replaced. Each cast consumes 1..20 bytes of
// the same 1 MiB oracle pool used by STR_CONCAT (candidate 65536 bytes).
//
// Profile 9 adds nominal enums with unit or one i64 payload, exact typed
// NEW_VARIANT/IS_VARIANT/UNWRAP_VARIANT operands and enum helper signatures.
// Qualified schemas and full tags are at most 128 ASCII bytes; 64 layouts,
// 32 variants per layout and 256 total variants are permitted. Empty layouts
// cannot construct a value. Record/enum nominal names cannot collide. Terminal
// enum names Option/Result (any case) and dotted spellings are reserved because
// the pinned VM canonicalizes those tag families (vm.cpp:435). Ordinary accepted
// tags remain byte-exact and case-sensitive. The actual EnumValue alternative
// carries nominal type, full tag and payload; every stack/local value is checked
// against declared metadata. Strings, scalars and other handles cannot forge it.
// The host checks payload kind, never computes or rewrites payload integers.
// Construction and copies retain all signed-i64 bits (vm.cpp:7309).
//
// UNWRAP deliberately retains the actual VM's min(requested,available) behavior
// (vm.cpp:7364), including zero values from a unit and discarding a payload when
// zero is requested. The provenance monitor tracks that actual stack shape.
// Candidate exact-shape enum_shape refusals are separate finite policy contrasts,
// never ordinary matching semantics or an invented VM fault/default. No enum
// allocation arena or cumulative descriptor policy is added to this inline type.

#include "nil_loader.hpp"
#include "nil_contract.hpp"
#include "vm.hpp"

#include <charconv>
#include <cerrno>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <fcntl.h>
#include <iostream>
#include <locale>
#include <optional>
#include <sstream>
#include <stdexcept>
#include <streambuf>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <unordered_map>
#include <unordered_set>
#include <variant>
#include <vector>

namespace {

constexpr std::size_t maximum_edition_bytes = 64 * 1024;
constexpr std::size_t maximum_instructions = 4096;
constexpr std::size_t maximum_steps = 1000000;
constexpr std::size_t maximum_frames = 64;
constexpr std::size_t maximum_stack_values = 4096;
constexpr const char *fallthrough_operand = "$ny.generated.nonvoid-fallthrough";
constexpr std::size_t maximum_output_bytes = 1024 * 1024;
constexpr std::size_t maximum_computed_text_bytes = 1024 * 1024;
constexpr std::size_t maximum_live_text_bytes = 16 * 1024 * 1024;
constexpr std::size_t maximum_array_literals = 1024;
constexpr std::size_t maximum_array_elements = 16384;
constexpr std::size_t maximum_array_literal_elements = 4096;
constexpr std::size_t maximum_array_text_bytes = 16 * 1024 * 1024;
constexpr std::size_t maximum_record_objects = 1024;
constexpr std::size_t maximum_record_cells = 16384;
constexpr std::size_t maximum_global_slots = 1024;

class file_handle {
public:
    explicit file_handle(const char *path)
        : descriptor_(::open(path, O_RDONLY | O_CLOEXEC | O_NONBLOCK)) {
        if (descriptor_ < 0) {
            throw std::runtime_error("cannot open edition: " +
                                     std::string(std::strerror(errno)));
        }
    }

    ~file_handle() { ::close(descriptor_); }
    file_handle(const file_handle &) = delete;
    file_handle &operator=(const file_handle &) = delete;
    int descriptor() const { return descriptor_; }

private:
    int descriptor_;
};

std::string read_edition(const char *path) {
    file_handle file(path);
    struct stat metadata {};
    if (::fstat(file.descriptor(), &metadata) != 0 ||
        !S_ISREG(metadata.st_mode) || metadata.st_size <= 0 ||
        static_cast<std::uint64_t>(metadata.st_size) > maximum_edition_bytes) {
        throw std::runtime_error("edition must be a regular file of 1..65536 bytes");
    }
    std::string bytes;
    char chunk[4096];
    for (;;) {
        const auto count = ::read(file.descriptor(), chunk, sizeof(chunk));
        if (count < 0 && errno == EINTR) {
            continue;
        }
        if (count < 0) {
            throw std::runtime_error("cannot read edition");
        }
        if (count == 0) {
            break;
        }
        if (static_cast<std::size_t>(count) > maximum_edition_bytes - bytes.size()) {
            throw std::runtime_error("edition exceeds 65536 bytes");
        }
        bytes.append(chunk, static_cast<std::size_t>(count));
    }
    if (bytes.size() != static_cast<std::size_t>(metadata.st_size)) {
        throw std::runtime_error("edition size changed while reading");
    }
    return bytes;
}

bool console_builtin(const std::string &name) {
    return name == "println" || name == "core__print" ||
           name == "console__writeln" || name == "console__write" ||
           name == "console__sys_write";
}

bool integer_width(const std::string &type) {
    return type == "i32" || type == "i64";
}

bool signed_decimal(const std::string &text) {
    const auto first_digit = !text.empty() && text[0] == '-' ? 1U : 0U;
    if (first_digit == text.size()) {
        return false;
    }
    for (std::size_t i = first_digit; i < text.size(); ++i) {
        if (text[i] < '0' || text[i] > '9') {
            return false;
        }
    }
    std::int64_t value = 0;
    const auto parsed = std::from_chars(text.data(), text.data() + text.size(), value);
    return parsed.ec == std::errc{} && parsed.ptr == text.data() + text.size();
}

// A lexical guard only: execution still uses the qualified VM's std::stoll.
// Optional sign, a real hexadecimal prefix, at least one ASCII digit, and a
// complete signed-i64 magnitude. Decimal acceptance remains unchanged.
bool signed_hexadecimal(const std::string &text) {
    std::size_t index = 0;
    const bool negative = !text.empty() && text[0] == '-';
    if (!text.empty() && (text[0] == '-' || text[0] == '+')) {
        ++index;
    }
    if (text.size() - index < 3 || text[index] != '0' ||
        (text[index + 1] != 'x' && text[index + 1] != 'X')) {
        return false;
    }
    index += 2;
    const auto limit = negative ? (std::uint64_t{1} << 63) :
                                 (std::uint64_t{1} << 63) - 1;
    std::uint64_t value = 0;
    for (; index < text.size(); ++index) {
        const unsigned char byte = text[index];
        const unsigned digit = byte >= '0' && byte <= '9' ? byte - '0' :
                               byte >= 'a' && byte <= 'f' ? byte - 'a' + 10 :
                               byte >= 'A' && byte <= 'F' ? byte - 'A' + 10 : 16;
        if (digit == 16 || value > (limit - digit) / 16) {
            return false;
        }
        value = value * 16 + digit;
    }
    return true;
}

bool value_type(const std::string &type) {
    return integer_width(type) || type == "text" ||
           type == "array<i64>" || type == "array<text>";
}

bool safe_name(const std::string &name) {
    const auto initial = [](unsigned char byte) {
        return (byte >= 'a' && byte <= 'z') || (byte >= 'A' && byte <= 'Z') || byte == '_';
    };
    if (name.empty() || name.size() > 128 || !initial(name.front())) {
        return false;
    }
    for (const unsigned char byte : name) {
        if (!(initial(byte) || (byte >= '0' && byte <= '9'))) {
            return false;
        }
    }
    return true;
}

bool qualified_name(const std::string &name) {
    if (name.empty() || name.size() > 128) {
        return false;
    }
    std::size_t start = 0;
    for (;;) {
        const auto end = name.find("::", start);
        if (!safe_name(name.substr(start, end == std::string::npos ? end : end - start))) {
            return false;
        }
        if (end == std::string::npos) {
            return true;
        }
        start = end + 2;
    }
}

bool nominal_name(const std::string &name) {
    return qualified_name(name) && !ny::is_reserved_nil_class_name(name);
}

// Resource accounting only. Formatting and produced bytes belong to the VM.
std::size_t decimal_bytes(std::int64_t value) {
    auto magnitude = static_cast<std::uint64_t>(value);
    std::size_t size = value < 0 ? 1 : 0;
    if (value < 0) {
        magnitude = std::uint64_t{0} - magnitude;
    }
    do {
        ++size;
        magnitude /= 10;
    } while (magnitude != 0);
    return size;
}

struct record_layout {
    std::string name;
    std::unordered_set<std::string> fields;
};

std::vector<record_layout> guard_record_layouts(const ny::nil_module &module) {
    const auto refuse = []() {
        throw std::runtime_error("reference record probe: outside profile-7 schema guard");
    };
    if (module.structs.size() > 64 || !module.classes.empty() ||
        !module.dependencies.empty()) {
        refuse();
    }
    std::vector<record_layout> layouts;
    std::unordered_set<std::string> names;
    std::size_t fields = 0;
    for (const auto &schema : module.structs) {
        if (!nominal_name(schema.name) || !names.insert(schema.name).second || schema.fields.size() > 32 ||
            schema.fields.size() > 256 - fields) {
            refuse();
        }
        record_layout layout{schema.name, {}};
        for (const auto &field : schema.fields) {
            if (!safe_name(field.first) || field.second != "i64" || !layout.fields.insert(field.first).second) {
                refuse();
            }
        }
        fields += schema.fields.size();
        layouts.push_back(std::move(layout));
    }
    return layouts;
}

std::optional<std::size_t> record_type(const std::vector<record_layout> &layouts, const std::string &name) {
    for (std::size_t index = 0; index < layouts.size(); ++index) {
        if (layouts[index].name == name) {
            return index;
        }
    }
    return std::nullopt;
}

struct enum_variant {
    std::string name;
    bool payload;
};

struct enum_layout {
    std::string name;
    std::vector<enum_variant> variants;
};

bool enum_name(const std::string &name) {
    if (!nominal_name(name)) {
        return false;
    }
    const auto separator = name.rfind("::");
    auto terminal = name.substr(separator == std::string::npos ? 0 : separator + 2);
    for (auto &byte : terminal) {
        if (byte >= 'A' && byte <= 'Z') {
            byte = static_cast<char>(byte - 'A' + 'a');
        }
    }
    return terminal != "option" && terminal != "result";
}

std::vector<enum_layout> guard_enum_layouts(const ny::nil_module &module,
                                          const std::vector<record_layout> &records) {
    const auto refuse = []() {
        throw std::runtime_error("reference enum probe: outside profile-9 schema guard");
    };
    if (module.enums.size() > 64) {
        refuse();
    }
    std::vector<enum_layout> layouts;
    std::unordered_set<std::string> names;
    std::size_t variants = 0;
    for (const auto &schema : module.enums) {
        if (!enum_name(schema.name) || !names.insert(schema.name).second || record_type(records, schema.name) ||
            schema.variants.size() > 32 || schema.variants.size() > 256 - variants) {
            refuse();
        }
        enum_layout layout{schema.name, {}};
        std::unordered_set<std::string> tags;
        for (const auto &variant : schema.variants) {
            if (!safe_name(variant.name) || schema.name.size() + 2 + variant.name.size() > 128 ||
                !tags.insert(variant.name).second ||
                (!variant.associated_type.empty() && variant.associated_type != "i64")) {
                refuse();
            }
            layout.variants.push_back({variant.name, variant.associated_type == "i64"});
        }
        variants += schema.variants.size();
        layouts.push_back(std::move(layout));
    }
    return layouts;
}

std::optional<std::size_t> enum_type(const std::vector<enum_layout> &layouts, const std::string &name) {
    for (std::size_t index = 0; index < layouts.size(); ++index) {
        if (layouts[index].name == name) {
            return index;
        }
    }
    return std::nullopt;
}

const enum_variant *enum_tag(const enum_layout &layout, const std::string &tag) {
    for (const auto &variant : layout.variants) {
        if (tag == layout.name + "::" + variant.name) {
            return &variant;
        }
    }
    return nullptr;
}

bool local_name(const std::string &name) {
    if (safe_name(name)) {
        return true;
    }
    constexpr const char *prefix = "$ny.temp.";
    constexpr std::size_t prefix_size = 9;
    if (name.size() > 128 || name.compare(0, prefix_size, prefix) != 0) {
        return false;
    }
    const auto separator = name.find('.', prefix_size);
    if (separator == std::string::npos ||
        !safe_name(name.substr(prefix_size, separator - prefix_size))) {
        return false;
    }
    const auto suffix = name.substr(separator + 1);
    if (suffix.empty() || (suffix.size() > 1 && suffix.front() == '0')) {
        return false;
    }
    for (const unsigned char byte : suffix) {
        if (byte < '0' || byte > '9') {
            return false;
        }
    }
    return true;
}

std::optional<std::size_t> argument_count(const std::string &text) {
    if (text.empty()) {
        return std::nullopt;
    }
    for (const char byte : text) {
        if (byte < '0' || byte > '9') {
            return std::nullopt;
        }
    }
    std::size_t count = 0;
    const auto parsed = std::from_chars(text.data(), text.data() + text.size(), count);
    if (parsed.ec != std::errc{} || parsed.ptr != text.data() + text.size() || count > 8) {
        return std::nullopt;
    }
    return count;
}

std::optional<std::size_t> array_literal_count(const std::string &text) {
    if (text.empty()) {
        return std::nullopt;
    }
    for (const char byte : text) {
        if (byte < '0' || byte > '9') {
            return std::nullopt;
        }
    }
    std::size_t count = 0;
    const auto parsed = std::from_chars(text.data(), text.data() + text.size(), count);
    if (parsed.ec != std::errc{} || parsed.ptr != text.data() + text.size() ||
        count > maximum_array_literal_elements) {
        return std::nullopt;
    }
    return count;
}

std::optional<std::size_t> enum_count(const std::string &text) {
    const auto count = argument_count(text);
    return count && *count <= 1 ? count : std::nullopt;
}

bool allowed_body_instruction(const ny::Instruction &instruction) {
    using ny::Opcode;
    const auto &args = instruction.args;
    if (instruction.is_function_entry || instruction.is_function_end) {
        return false;
    }
    switch (instruction.op) {
    case Opcode::PUSH_CONST:
        return args.size() == 1 && (signed_decimal(args[0]) || signed_hexadecimal(args[0]));
    case Opcode::PUSH_FN_PTR:
        return args.size() == 1 && safe_name(args[0]);
    case Opcode::NEW_VARIANT:
        return args.size() == 4 && qualified_name(args[0]) && enum_count(args[1]) &&
               enum_name(args[2]) && args[3] == (*enum_count(args[1]) == 0 ? "void" : "i64");
    case Opcode::IS_VARIANT:
        return args.size() == 3 && qualified_name(args[0]) && enum_name(args[1]) &&
               (args[2] == "void" || args[2] == "i64");
    case Opcode::UNWRAP_VARIANT:
        return args.size() == 3 && enum_count(args[0]) && enum_name(args[1]) &&
               args[2] == (*enum_count(args[0]) == 0 ? "void" : "i64");
    case Opcode::LOAD_GLOBAL:
    case Opcode::STORE_GLOBAL:
        return args.size() == 2 && qualified_name(args[0]) && args[1] == "i64";
    case Opcode::NEW_STRUCT:
        return args.size() == 1 && nominal_name(args[0]);
    case Opcode::GET_FIELD:
    case Opcode::SET_FIELD:
        return args.size() == 1 && safe_name(args[0]);
    case Opcode::NEW_ARRAY_LITERAL:
        return args.size() == 2 && array_literal_count(args[0]).has_value() &&
               (args[1] == "i64" || args[1] == "text");
    case Opcode::LOAD_LOCAL:
    case Opcode::STORE_LOCAL:
        return args.size() == 1 && local_name(args[0]);
    case Opcode::PUSH_CONST_STR:
    case Opcode::LABEL:
    case Opcode::JMP:
    case Opcode::JMP_IF:
    case Opcode::JMP_IF_NOT:
        return args.size() == 1 && safe_name(args[0]);
    case Opcode::ADD:
        return args.empty() || (args.size() == 1 && integer_width(args[0]));
    case Opcode::CAST:
        return args.size() == 1 && (integer_width(args[0]) || args[0] == "text");
    case Opcode::NEG:
    case Opcode::SUB:
    case Opcode::MUL:
    case Opcode::DIV:
    case Opcode::MOD:
    case Opcode::BIT_AND:
    case Opcode::BIT_OR:
    case Opcode::BIT_XOR:
    case Opcode::SHL:
    case Opcode::SHR:
        return args.size() == 1 && integer_width(args[0]);
    case Opcode::CMP_EQ:
    case Opcode::CMP_NE:
    case Opcode::CMP_LT:
    case Opcode::CMP_LE:
    case Opcode::CMP_GT:
    case Opcode::CMP_GE:
    case Opcode::STR_CONCAT:
    case Opcode::LEN:
    case Opcode::GET_INDEX:
    case Opcode::NOT:
    case Opcode::POP:
    case Opcode::NOP:
    case Opcode::CONSOLE_WRITE:
    case Opcode::CONSOLE_WRITELN:
        return args.empty();
    case Opcode::RET:
        return args.empty() || (args.size() == 1 && args[0] == fallthrough_operand);
    case Opcode::CALL:
        return args.size() == 2 && local_name(args[0]) && argument_count(args[1]).has_value();
    case Opcode::INTEROP_CALL:
        return args.size() == 4 && args[0] == "intrinsic" &&
               (args[1] == "console.write" || args[1] == "console.writeln") &&
               args[2] == "1" && args[3] == "void";
    default:
        return false;
    }
}

struct function_range {
    std::string name;
    std::size_t parameters;
    std::size_t begin;
    std::size_t end;
    std::string return_type;
    std::vector<std::string> parameter_types;
    std::unordered_set<std::string> locals;
};

std::vector<function_range> guard_effects(const std::vector<ny::Instruction> &program,
                  const std::unordered_map<std::string, ny::Value> &constants,
                  const ny::nil_module &module, const std::vector<record_layout> &layouts,
                  const std::vector<enum_layout> &enumerations) {
    using ny::Opcode;
    const auto refuse = []() {
        throw std::runtime_error("reference enum probe: outside profile-9 effect guard");
    };
    if (constants.size() > 128 || program.size() > maximum_instructions + 64 * 3) {
        refuse();
    }
    for (const auto &constant : constants) {
        if (!safe_name(constant.first) || !std::holds_alternative<std::string>(constant.second)) {
            refuse();
        }
    }
    std::vector<function_range> functions;
    std::unordered_map<std::string, std::size_t> arities;
    // The VM has one global label map, including function-entry/end labels.
    std::unordered_set<std::string> all_labels;
    std::size_t instructions = 0;
    std::size_t pc = 0;
    while (pc < program.size()) {
        const auto skip_label = "$ny.internal.function-end." + std::to_string(functions.size());
        const auto &skip = program[pc++];
        if (skip.op != Opcode::JMP || skip.args != std::vector<std::string>{skip_label} ||
            skip.is_function_entry || skip.is_function_end || pc == program.size()) {
            refuse();
        }
        const auto &entry = program[pc++];
        if (entry.op != Opcode::LABEL || !entry.is_function_entry || entry.is_function_end ||
            entry.args.size() != 1 || !safe_name(entry.args[0]) ||
            entry.function_param_count > 8 ||
            entry.function_param_count != entry.function_parameter_types.size() ||
            !(value_type(entry.function_return_type) || record_type(layouts, entry.function_return_type) ||
              enum_type(enumerations, entry.function_return_type) ||
              entry.function_return_type == "void") ||
            console_builtin(entry.args[0]) || functions.size() == 64 ||
            !arities.emplace(entry.args[0], entry.function_param_count).second ||
            !all_labels.insert(entry.args[0]).second) {
            refuse();
        }
        if (entry.args[0] == "main" &&
            (entry.function_param_count != 0 || !integer_width(entry.function_return_type))) {
            refuse();
        }
        for (const auto &type : entry.function_parameter_types) {
            if (!value_type(type) && !record_type(layouts, type) && !enum_type(enumerations, type)) {
                refuse();
            }
        }
        const auto begin = pc;
        while (pc < program.size() && !program[pc].is_function_end) {
            const auto &instruction = program[pc++];
            if (!allowed_body_instruction(instruction) || ++instructions > maximum_instructions) {
                refuse();
            }
            if (instruction.op == Opcode::LABEL && !all_labels.insert(instruction.args[0]).second) {
                refuse();
            }
        }
        const auto end_pc = pc;
        if (pc == program.size()) {
            refuse();
        }
        const auto &end = program[pc++];
        if (end.op != Opcode::LABEL || end.is_function_entry ||
            end.args != std::vector<std::string>{skip_label} || !all_labels.insert(skip_label).second) {
            refuse();
        }
        functions.push_back({entry.args[0], entry.function_param_count, begin, end_pc,
                             entry.function_return_type, entry.function_parameter_types, {}});
    }
    if (arities.count("main") != 1) {
        refuse();
    }
    if (module.functions.size() != functions.size()) {
        refuse();
    }
    for (std::size_t index = 0; index < functions.size(); ++index) {
        auto &function = functions[index];
        const auto &declaration = module.functions[index];
        if (declaration.name != function.name || declaration.params.size() != function.parameters ||
            declaration.return_type != function.return_type) {
            refuse();
        }
        std::size_t parameter_index = 0;
        for (const auto &parameter : declaration.params) {
            const auto colon = parameter.find(':');
            if (colon == std::string::npos) {
                refuse();
            }
            auto name = parameter.substr(0, colon);
            const auto first = name.find_first_not_of(" \t");
            const auto last = name.find_last_not_of(" \t");
            if (first == std::string::npos) {
                refuse();
            }
            name = name.substr(first, last - first + 1);
            auto type = parameter.substr(colon + 1);
            const auto type_first = type.find_first_not_of(" \t");
            const auto type_last = type.find_last_not_of(" \t");
            if (type_first == std::string::npos ||
                type.substr(type_first, type_last - type_first + 1) !=
                    program[function.begin - 1].function_parameter_types[parameter_index++]) {
                refuse();
            }
            if (!safe_name(name) || !function.locals.insert(name).second ||
                arities.count(name) != 0 || console_builtin(name)) {
                refuse();
            }
        }
        std::unordered_set<std::string> local_labels;
        for (std::size_t index = function.begin; index < function.end; ++index) {
            const auto &instruction = program[index];
            if (instruction.op == Opcode::LABEL) {
                local_labels.insert(instruction.args[0]);
            }
            if (instruction.op == Opcode::LOAD_LOCAL || instruction.op == Opcode::STORE_LOCAL) {
                const auto &name = instruction.args[0];
                function.locals.insert(name);
                if (function.locals.size() > 128) {
                    refuse();
                }
            }
        }
        for (std::size_t index = function.begin; index < function.end; ++index) {
            const auto &instruction = program[index];
            const auto &args = instruction.args;
            if (instruction.op == Opcode::JMP || instruction.op == Opcode::JMP_IF ||
                instruction.op == Opcode::JMP_IF_NOT) {
                if (local_labels.count(args[0]) != 1) {
                    refuse();
                }
            } else if (instruction.op == Opcode::CALL) {
                const auto count = argument_count(args[1]);
                const auto declared = arities.find(args[0]);
                if (!count || (function.locals.count(args[0]) != 0 ? false :
                    console_builtin(args[0]) ? *count != 1 :
                    declared == arities.end() || declared->second != *count)) {
                    refuse();
                }
            } else if (instruction.op == Opcode::NEW_VARIANT || instruction.op == Opcode::IS_VARIANT) {
                const auto layout_index = enum_type(enumerations, args[instruction.op == Opcode::NEW_VARIANT ? 2 : 1]);
                const auto *variant = layout_index ? enum_tag(enumerations[*layout_index], args[0]) : nullptr;
                const auto &payload = args[instruction.op == Opcode::NEW_VARIANT ? 3 : 2];
                if (!variant || payload != (variant->payload ? "i64" : "void")) {
                    refuse();
                }
            } else if (instruction.op == Opcode::UNWRAP_VARIANT) {
                const auto layout_index = enum_type(enumerations, args[1]);
                bool shape = false;
                if (layout_index) {
                    for (const auto &variant : enumerations[*layout_index].variants) {
                        shape = shape || variant.payload == (*enum_count(args[0]) == 1);
                    }
                }
                if (!shape) {
                    refuse();
                }
            } else if (instruction.op == Opcode::NEW_STRUCT && !record_type(layouts, args[0])) {
                refuse();
            } else if (instruction.op == Opcode::GET_FIELD || instruction.op == Opcode::SET_FIELD) {
                bool known = false;
                for (const auto &layout : layouts) {
                    known = known || layout.fields.count(args[0]) != 0;
                }
                if (!known) {
                    refuse();
                }
            } else if (instruction.op == Opcode::PUSH_FN_PTR && arities.count(args[0]) != 1) {
                refuse();
            } else if (instruction.op == Opcode::PUSH_CONST_STR && constants.count(args[0]) != 1) {
                refuse();
            }
        }
    }
    return functions;
}

std::unordered_set<std::string> guard_global_names(const std::vector<ny::Instruction> &program) {
    std::unordered_set<std::string> globals;
    for (const auto &instruction : program) {
        if (instruction.op == ny::Opcode::LOAD_GLOBAL || instruction.op == ny::Opcode::STORE_GLOBAL) {
            globals.insert(instruction.args[0]);
            if (globals.size() > maximum_global_slots) {
                throw std::runtime_error("reference global probe: global name limit");
            }
        }
    }
    return globals;
}

// This monitor carries provenance, not NIL values. The actual VM remains the
// only implementation of opcode semantics. A drift is an oracle refusal.
class callable_provenance {
    using tag = std::optional<std::size_t>;
    struct frame_tags {
        std::size_t function;
        std::size_t floor;
        std::unordered_map<std::string, tag> locals;
    };
    struct pending_call {
        std::size_t function;
        std::size_t arguments;
    };

public:
    explicit callable_provenance(const std::vector<function_range> &functions)
        : functions_(functions) {}

    std::optional<std::size_t> dispatched_target() const { return dispatched_target_; }
    std::size_t current_floor() const { return frames_.back().floor; }

    // Returns whether this particular CALL dispatches a console builtin.
    bool before(ny::VM &running) {
        dispatched_target_.reset();
        const auto &actual = running.getStack();
        const auto depth = running.getCallDepth();
        const auto function = function_at(running.getPC());
        if (frames_.empty()) {
            if (depth != 1 || functions_[function].name != "main" || !actual.empty()) {
                refuse("initial frame drift");
            }
            frames_.push_back({function, 0, {}});
        } else if (entering_) {
            if (depth != frames_.size() + 1 || function != entering_->function ||
                entering_->arguments > actual.size()) {
                refuse("call frame drift");
            }
            frames_.push_back({function, actual.size() - entering_->arguments, {}});
            entering_.reset();
        } else if (leaving_) {
            if (frames_.size() <= 1 || depth + 1 != frames_.size()) {
                refuse("return frame drift");
            }
            frames_.pop_back();
            leaving_ = false;
        }
        if (depth != frames_.size() || function != frames_.back().function ||
            actual.size() != stack_.size()) {
            refuse("stack/frame drift");
        }
        for (std::size_t index = 0; index < stack_.size(); ++index) {
            check_tag(actual[index], stack_[index]);
        }
        std::size_t frame_index = 0;
        running.visitDebugFrames(maximum_frames, 128 * maximum_frames,
            [&](const std::string &name, std::size_t,
                const std::unordered_map<std::string, ny::Value> &locals) {
                if (frame_index >= frames_.size()) {
                    refuse("local frame drift");
                }
                const auto &frame = frames_[frame_index++];
                if (name != functions_[frame.function].name || locals.size() != frame.locals.size()) {
                    refuse("local frame drift");
                }
                for (const auto &local : frame.locals) {
                    const auto found = locals.find(local.first);
                    if (found == locals.end()) {
                        refuse("missing local provenance");
                    }
                    check_tag(found->second, local.second);
                }
            });
        if (frame_index != frames_.size()) {
            refuse("local frame count drift");
        }

        using ny::Opcode;
        const auto &instruction = running.getProgram()[running.getPC()];
        const auto &args = instruction.args;
        switch (instruction.op) {
        case Opcode::PUSH_FN_PTR:
            stack_.push_back(find_function(args[0]));
            break;
        case Opcode::PUSH_CONST:
        case Opcode::PUSH_CONST_STR:
        case Opcode::NEW_STRUCT:
        case Opcode::LOAD_GLOBAL:
            stack_.push_back(std::nullopt);
            break;
        case Opcode::LOAD_LOCAL: {
            const auto found = frames_.back().locals.find(args[0]);
            if (found == frames_.back().locals.end()) {
                refuse("uninitialized local");
            }
            stack_.push_back(found->second);
            break;
        }
        case Opcode::STORE_LOCAL:
            require_values(1);
            frames_.back().locals[args[0]] = stack_.back();
            stack_.pop_back();
            break;
        case Opcode::POP:
            require_values(1);
            stack_.pop_back();
            break;
        case Opcode::CALL: {
            const auto count = *argument_count(args[1]);
            require_noncallable(count);
            std::optional<std::size_t> target;
            if (functions_[function].locals.count(args[0]) != 0) {
                const auto local = frames_.back().locals.find(args[0]);
                if (local == frames_.back().locals.end() || !local->second) {
                    refuse("local CALL requires callable provenance");
                }
                target = local->second;
            } else if (!console_builtin(args[0])) {
                target = find_function(args[0]);
            }
            if (target) {
                if (functions_[*target].parameters != count) {
                    refuse("callable arity mismatch");
                }
                if (depth == maximum_frames) {
                    refuse("frame limit");
                }
                // VM pushFrame leaves the arguments on the shared stack for
                // the callee's explicit STORE_LOCAL prologue.
                dispatched_target_ = target;
                entering_ = pending_call{*target, count};
                return false;
            }
            stack_.resize(stack_.size() - count);
            return true;
        }
        case Opcode::RET:
            require_values(0);
            require_noncallable(stack_.size() - frames_.back().floor);
            leaving_ = frames_.size() > 1;
            break;
        case Opcode::NEW_VARIANT:
            consume(*enum_count(args[1]), true);
            break;
        case Opcode::IS_VARIANT:
            consume(1, true);
            break;
        case Opcode::UNWRAP_VARIANT: {
            require_noncallable(1);
            const auto *enumeration = std::get_if<ny::EnumValue>(&actual.back());
            const bool produced = *enum_count(args[0]) != 0 && enumeration && !enumeration->payloads.empty();
            consume(1, produced);
            break;
        }
        case Opcode::STORE_GLOBAL:
            consume(1, false);
            break;
        case Opcode::SET_FIELD:
            require_noncallable(2);
            stack_.pop_back();
            break;
        case Opcode::GET_FIELD:
            consume(1, true);
            break;
        case Opcode::NEW_ARRAY_LITERAL:
            consume(*array_literal_count(args[0]), true);
            break;
        case Opcode::ADD: case Opcode::SUB: case Opcode::MUL:
        case Opcode::DIV: case Opcode::MOD: case Opcode::BIT_AND:
        case Opcode::BIT_OR: case Opcode::BIT_XOR: case Opcode::SHL:
        case Opcode::SHR: case Opcode::CMP_EQ: case Opcode::CMP_NE:
        case Opcode::CMP_LT: case Opcode::CMP_LE: case Opcode::CMP_GT:
        case Opcode::CMP_GE: case Opcode::STR_CONCAT: case Opcode::GET_INDEX:
            consume(2, true);
            break;
        case Opcode::CAST: case Opcode::NEG: case Opcode::NOT: case Opcode::LEN:
            consume(1, true);
            break;
        case Opcode::JMP_IF: case Opcode::JMP_IF_NOT:
        case Opcode::CONSOLE_WRITE: case Opcode::CONSOLE_WRITELN:
        case Opcode::INTEROP_CALL:
            consume(1, false);
            break;
        case Opcode::LABEL: case Opcode::JMP: case Opcode::NOP:
            break;
        default:
            refuse("unexpected provenance opcode");
        }
        return false;
    }

private:
    [[noreturn]] static void refuse(const char *message) {
        throw std::runtime_error(std::string("reference callable probe: ") + message);
    }
    std::size_t find_function(const std::string &name) const {
        for (std::size_t index = 0; index < functions_.size(); ++index) {
            if (functions_[index].name == name) {
                return index;
            }
        }
        refuse("unknown internal target");
    }
    std::size_t function_at(std::size_t pc) const {
        for (std::size_t index = 0; index < functions_.size(); ++index) {
            // Main dispatches its generated entry LABEL once; helper CALL
            // skips it. Both boundaries belong to the same declared function.
            if (pc >= functions_[index].begin - 1 && pc < functions_[index].end) {
                return index;
            }
        }
        refuse("instruction outside function");
    }
    void check_tag(const ny::Value &value, tag provenance) const {
        if (provenance) {
            const auto *name = std::get_if<std::string>(&value);
            if (*provenance >= functions_.size() || !name || *name != functions_[*provenance].name) {
                refuse("callable value drift");
            }
        }
    }
    void require_values(std::size_t count) const {
        if (frames_.back().floor > stack_.size() || count > stack_.size() - frames_.back().floor) {
            refuse("operand provenance underflow");
        }
    }
    void require_noncallable(std::size_t count) const {
        require_values(count);
        for (std::size_t index = stack_.size() - count; index < stack_.size(); ++index) {
            if (stack_[index]) {
                refuse("callable used as ordinary value");
            }
        }
    }
    void consume(std::size_t count, bool produces_value) {
        require_noncallable(count);
        stack_.resize(stack_.size() - count);
        if (produces_value) {
            stack_.push_back(std::nullopt);
        }
    }
    const std::vector<function_range> &functions_;
    std::vector<tag> stack_;
    std::vector<frame_tags> frames_;
    std::optional<pending_call> entering_;
    std::optional<std::size_t> dispatched_target_;
    bool leaving_ = false;
};

// Record values already have a distinct VM alternative. Track their real shared
// identity instead of duplicating fields or arithmetic in a shadow interpreter.
class record_provenance {
    struct allocation {
        std::size_t layout;
        std::shared_ptr<ny::StructData> data;
    };

public:
    record_provenance(const std::vector<record_layout> &layouts,
                      const std::vector<function_range> &functions)
        : layouts_(layouts), functions_(functions) {}

    void before(ny::VM &running, std::optional<std::size_t> target, std::size_t floor) {
        const auto &stack = running.getStack();
        if (pending_) {
            if (stack.empty()) {
                refuse("allocation stack drift");
            }
            const auto *record = std::get_if<ny::StructValue>(&stack.back());
            if (!record || record->type_name != layouts_[*pending_].name || !record->data ||
                !record->data->fields.empty() || identities_.count(record->data.get()) != 0) {
                refuse("allocation identity drift");
            }
            const auto index = allocations_.size();
            allocations_.push_back({*pending_, record->data});
            identities_.emplace(record->data.get(), index);
            pending_.reset();
        }
        for (const auto &value : stack) {
            check_identity(value);
        }
        running.visitDebugFrames(maximum_frames, 128 * maximum_frames,
            [this](const std::string &, std::size_t,
                   const std::unordered_map<std::string, ny::Value> &locals) {
                for (const auto &local : locals) {
                    check_identity(local.second);
                }
            });

        using ny::Opcode;
        const auto &instruction = running.getProgram()[running.getPC()];
        const auto &args = instruction.args;
        const auto require = [&stack](std::size_t count) {
            if (count > stack.size()) {
                refuse("record operand underflow");
            }
        };
        switch (instruction.op) {
        case Opcode::NEW_STRUCT: {
            const auto layout = record_type(layouts_, args[0]);
            if (!layout) {
                refuse("undeclared record allocation");
            }
            const auto cells = layouts_[*layout].fields.size();
            if (allocations_.size() == maximum_record_objects ||
                cells > maximum_record_cells - allocated_cells_) {
                refuse("record allocation limit");
            }
            allocated_cells_ += cells;
            pending_ = layout;
            break;
        }
        case Opcode::GET_FIELD:
            require(1);
            require_field(stack.back(), args[0]);
            break;
        case Opcode::SET_FIELD:
            require(2);
            require_field(stack[stack.size() - 2], args[0]);
            if (!std::holds_alternative<std::int64_t>(stack.back())) {
                refuse("SET_FIELD requires i64 value");
            }
            break;
        case Opcode::CALL:
            if (target) {
                const auto &types = functions_[*target].parameter_types;
                require(types.size());
                for (std::size_t index = 0; index < types.size(); ++index) {
                    require_type(stack[stack.size() - types.size() + index], types[index]);
                }
            } else {
                reject_records(stack, *argument_count(args[1]));
            }
            break;
        case Opcode::RET: {
            const auto &function = function_at(running.getPC());
            // The VM owns empty/generated-fallthrough return faults. Check
            // nominal type only when the current frame actually has a value.
            if (stack.size() > floor) {
                require_type(stack.back(), function.return_type);
            }
            break;
        }
        case Opcode::STORE_GLOBAL:
            reject_records(stack, 1);
            break;
        case Opcode::NEW_ARRAY_LITERAL:
            reject_records(stack, *array_literal_count(args[0]));
            break;
        case Opcode::ADD: case Opcode::SUB: case Opcode::MUL:
        case Opcode::DIV: case Opcode::MOD: case Opcode::BIT_AND:
        case Opcode::BIT_OR: case Opcode::BIT_XOR: case Opcode::SHL:
        case Opcode::SHR: case Opcode::CMP_EQ: case Opcode::CMP_NE:
        case Opcode::CMP_LT: case Opcode::CMP_LE: case Opcode::CMP_GT:
        case Opcode::CMP_GE: case Opcode::STR_CONCAT: case Opcode::GET_INDEX:
            reject_records(stack, 2);
            break;
        case Opcode::CAST: case Opcode::NEG: case Opcode::NOT: case Opcode::LEN:
        case Opcode::JMP_IF: case Opcode::JMP_IF_NOT:
        case Opcode::CONSOLE_WRITE: case Opcode::CONSOLE_WRITELN:
        case Opcode::INTEROP_CALL:
            reject_records(stack, 1);
            break;
        default:
            break;
        }
    }

private:
    [[noreturn]] static void refuse(const char *message) {
        throw std::runtime_error(std::string("reference record probe: ") + message);
    }
    const function_range &function_at(std::size_t pc) const {
        for (const auto &function : functions_) {
            if (pc >= function.begin - 1 && pc < function.end) {
                return function;
            }
        }
        refuse("instruction outside function");
    }
    std::optional<std::size_t> check_identity(const ny::Value &value) const {
        const auto *record = std::get_if<ny::StructValue>(&value);
        if (!record) {
            return std::nullopt;
        }
        const auto found = identities_.find(record->data.get());
        if (!record->data || found == identities_.end()) {
            refuse("unregistered record identity");
        }
        const auto &allocation = allocations_[found->second];
        const auto &layout = layouts_[allocation.layout];
        if (record->type_name != layout.name || !record->static_class_view.empty() ||
            record->data != allocation.data || !record->data->array_fields.empty() ||
            !record->data->string_fields.empty() || !record->data->double_fields.empty() ||
            !record->data->struct_fields.empty() || !record->data->enum_fields.empty() ||
            !record->data->anything_fields.empty()) {
            refuse("record identity/type drift");
        }
        for (const auto &field : record->data->fields) {
            if (layout.fields.count(field.first) != 1) {
                refuse("undeclared stored field");
            }
        }
        return allocation.layout;
    }
    void require_field(const ny::Value &value, const std::string &name) const {
        const auto layout = check_identity(value);
        if (!layout || layouts_[*layout].fields.count(name) != 1) {
            refuse("field requires declared nominal owner");
        }
    }
    void require_type(const ny::Value &value, const std::string &type) const {
        const auto expected = record_type(layouts_, type);
        const auto actual = check_identity(value);
        if (expected != actual) {
            refuse("record argument/return nominal mismatch");
        }
    }
    void reject_records(const std::vector<ny::Value> &stack, std::size_t count) const {
        if (count > stack.size()) {
            refuse("record operand underflow");
        }
        for (std::size_t index = stack.size() - count; index < stack.size(); ++index) {
            if (std::holds_alternative<ny::StructValue>(stack[index])) {
                refuse("record used as scalar/array/text value");
            }
        }
    }
    const std::vector<record_layout> &layouts_;
    const std::vector<function_range> &functions_;
    std::vector<allocation> allocations_;
    std::unordered_map<const ny::StructData *, std::size_t> identities_;
    std::optional<std::size_t> pending_;
    std::size_t allocated_cells_ = 0;
};

// Enum identity is a nominal value, not a heap address. Inspect the VM's real
// EnumValue alternative and metadata; payload numbers are never mirrored.
class enum_provenance {
public:
    enum_provenance(const std::vector<enum_layout> &layouts,
                    const std::vector<function_range> &functions)
        : layouts_(layouts), functions_(functions) {}

    void before(ny::VM &running, std::optional<std::size_t> target, std::size_t floor) {
        const auto &stack = running.getStack();
        for (const auto &value : stack) {
            check_value(value);
        }
        running.visitDebugFrames(maximum_frames, 128 * maximum_frames,
            [this](const std::string &, std::size_t,
                   const std::unordered_map<std::string, ny::Value> &locals) {
                for (const auto &local : locals) {
                    check_value(local.second);
                }
            });
        using ny::Opcode;
        const auto &instruction = running.getProgram()[running.getPC()];
        const auto &args = instruction.args;
        switch (instruction.op) {
        case Opcode::NEW_VARIANT:
            if (*enum_count(args[1]) != 0 &&
                (stack.empty() || !std::holds_alternative<std::int64_t>(stack.back()))) {
                refuse("NEW_VARIANT requires i64 payload");
            }
            break;
        case Opcode::IS_VARIANT:
        case Opcode::UNWRAP_VARIANT:
            if (stack.empty()) {
                refuse("enum operand underflow");
            }
            require_type(stack.back(), args[1]);
            // No exact selected-shape assertion here: the actual VM owns
            // permissive unwrap semantics used by finite policy contrasts.
            break;
        case Opcode::CALL:
            if (target) {
                const auto &types = functions_[*target].parameter_types;
                if (types.size() > stack.size()) {
                    refuse("enum argument underflow");
                }
                for (std::size_t index = 0; index < types.size(); ++index) {
                    require_type(stack[stack.size() - types.size() + index], types[index]);
                }
            } else {
                reject_enums(stack, *argument_count(args[1]));
            }
            break;
        case Opcode::RET:
            if (stack.size() > floor) {
                require_type(stack.back(), function_at(running.getPC()).return_type);
            }
            break;
        case Opcode::NEW_ARRAY_LITERAL:
            reject_enums(stack, *array_literal_count(args[0]));
            break;
        case Opcode::ADD: case Opcode::SUB: case Opcode::MUL:
        case Opcode::DIV: case Opcode::MOD: case Opcode::BIT_AND:
        case Opcode::BIT_OR: case Opcode::BIT_XOR: case Opcode::SHL:
        case Opcode::SHR: case Opcode::CMP_EQ: case Opcode::CMP_NE:
        case Opcode::CMP_LT: case Opcode::CMP_LE: case Opcode::CMP_GT:
        case Opcode::CMP_GE: case Opcode::STR_CONCAT: case Opcode::GET_INDEX:
            reject_enums(stack, 2);
            break;
        case Opcode::CAST: case Opcode::NEG: case Opcode::NOT: case Opcode::LEN:
        case Opcode::JMP_IF: case Opcode::JMP_IF_NOT:
        case Opcode::CONSOLE_WRITE: case Opcode::CONSOLE_WRITELN:
        case Opcode::INTEROP_CALL: case Opcode::STORE_GLOBAL:
            reject_enums(stack, 1);
            break;
        default:
            break;
        }
    }

private:
    [[noreturn]] static void refuse(const char *message) {
        throw std::runtime_error(std::string("reference enum probe: ") + message);
    }
    const function_range &function_at(std::size_t pc) const {
        for (const auto &function : functions_) {
            if (pc >= function.begin - 1 && pc < function.end) {
                return function;
            }
        }
        refuse("instruction outside function");
    }
    std::optional<std::size_t> check_value(const ny::Value &value) const {
        const auto *enumeration = std::get_if<ny::EnumValue>(&value);
        if (!enumeration) {
            return std::nullopt;
        }
        const auto layout = enum_type(layouts_, enumeration->type_name);
        const auto *variant = layout ? enum_tag(layouts_[*layout], enumeration->variant_name) : nullptr;
        if (!variant || enumeration->payloads.size() != (variant->payload ? 1U : 0U) ||
            (variant->payload && !std::holds_alternative<std::int64_t>(enumeration->payloads[0]))) {
            refuse("enum nominal/tag/payload drift");
        }
        return layout;
    }
    void require_type(const ny::Value &value, const std::string &type) const {
        if (check_value(value) != enum_type(layouts_, type)) {
            refuse("enum receiver/argument/return nominal mismatch");
        }
    }
    void reject_enums(const std::vector<ny::Value> &stack, std::size_t count) const {
        if (count > stack.size()) {
            refuse("enum operand underflow");
        }
        for (std::size_t index = stack.size() - count; index < stack.size(); ++index) {
            if (std::holds_alternative<ny::EnumValue>(stack[index])) {
                refuse("enum used as scalar/array/text value");
            }
        }
    }
    const std::vector<enum_layout> &layouts_;
    const std::vector<function_range> &functions_;
};

class bounded_output : public std::streambuf {
public:
    const std::string &bytes() const { return bytes_; }

protected:
    std::streamsize xsputn(const char *bytes, std::streamsize count) override {
        if (count < 0 || static_cast<std::size_t>(count) > maximum_output_bytes - bytes_.size()) {
            throw std::runtime_error("reference probe: output exceeds 1048576 bytes");
        }
        bytes_.append(bytes, static_cast<std::size_t>(count));
        return count;
    }

    int_type overflow(int_type value) override {
        if (traits_type::eq_int_type(value, traits_type::eof())) {
            return traits_type::not_eof(value);
        }
        const char byte = traits_type::to_char_type(value);
        xsputn(&byte, 1);
        return value;
    }

private:
    std::string bytes_;
};

std::string hex_bytes(const std::string &bytes) {
    constexpr char digits[] = "0123456789abcdef";
    std::string result;
    result.reserve(bytes.size() * 2);
    for (const unsigned char byte : bytes) {
        result.push_back(digits[byte >> 4]);
        result.push_back(digits[byte & 15]);
    }
    return result;
}

std::string json_string(const std::string &text) {
    constexpr char digits[] = "0123456789abcdef";
    std::string result = "\"";
    for (const unsigned char byte : text) {
        if (byte == '"' || byte == '\\') {
            result.push_back('\\');
            result.push_back(static_cast<char>(byte));
        } else if (byte < 0x20 || byte >= 0x7f) {
            // The qualified VM's profile-4 diagnostics are ASCII. Escaping
            // every non-ASCII byte also keeps unexpected diagnostics valid JSON.
            result += "\\u00";
            result.push_back(digits[byte >> 4]);
            result.push_back(digits[byte & 15]);
        } else {
            result.push_back(static_cast<char>(byte));
        }
    }
    result.push_back('"');
    return result;
}

} // namespace

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "usage: program_enum_reference EDITION_PATH\n";
        return 2;
    }
    std::string edition;
    try {
        edition = read_edition(argv[1]);
        if (::unsetenv("NIL_MEMORY_PROFILE") != 0) {
            throw std::runtime_error("cannot disable memory profiling");
        }
        if (::unsetenv("NIL_STRICT_BOUNDS") != 0) {
            throw std::runtime_error("cannot disable diagnostic strict bounds");
        }
    } catch (const std::exception &error) {
        std::cerr << "reference probe: " << error.what() << '\n';
        return 2;
    }
    bounded_output capture;
    std::ostream output(&capture);
    output.imbue(std::locale::classic());
    output.exceptions(std::ios::badbit | std::ios::failbit);
    std::optional<std::int64_t> result;
    std::optional<std::string> error;
    try {
        std::istringstream input(edition);
        std::unordered_map<std::string, ny::Value> constants;
        const auto program = ny::parseProgram(input, constants);
        std::istringstream declaration_input(edition);
        const auto declarations = ny::parseTextNilModule(declaration_input);
        if (!declarations) {
            throw std::runtime_error("reference callable probe: missing parsed declarations");
        }
        const auto layouts = guard_record_layouts(*declarations);
        const auto enum_layouts = guard_enum_layouts(*declarations, layouts);
        const auto functions = guard_effects(program, constants, *declarations, layouts, enum_layouts);
        callable_provenance provenance(functions);
        record_provenance records(layouts, functions);
        enum_provenance enumerations(enum_layouts, functions);
        const auto globals = guard_global_names(program);
        ny::VM vm;
        vm.setOutputStream(output);
        vm.setProcessExitTrapping(true);
        vm.setStrictBounds(false);
        std::size_t steps = 0;
        std::size_t computed_text_bytes = 0;
        std::size_t array_literals = 0;
        std::size_t array_elements = 0;
        std::size_t array_text_bytes = 0;
        vm.setControlCallback([&steps, &computed_text_bytes, &array_literals,
                               &array_elements, &array_text_bytes, &provenance, &records, &enumerations, &globals](ny::VM &running) {
            if (steps == maximum_steps) {
                throw std::runtime_error("reference text probe: instruction limit");
            }
            ++steps;
            if (running.getCallDepth() > maximum_frames ||
                running.getStack().size() > maximum_stack_values) {
                throw std::runtime_error("reference text probe: frame/stack limit");
            }
            // The debug visitor borrows the VM's actual locals without copying
            // strings. Include literal and computed values, including aliases
            // represented as independent std::string copies in the reference.
            std::size_t live_text_bytes = 0;
            const auto account_text = [&live_text_bytes](const ny::Value &value) {
                if (const auto *text = std::get_if<std::string>(&value)) {
                    if (text->size() > maximum_live_text_bytes - live_text_bytes) {
                        throw std::runtime_error("reference text probe: live text limit");
                    }
                    live_text_bytes += text->size();
                }
            };
            for (const auto &value : running.getStack()) {
                account_text(value);
            }
            running.visitDebugFrames(maximum_frames, 128 * maximum_frames,
                [&account_text](const std::string &, std::size_t,
                                const std::unordered_map<std::string, ny::Value> &locals) {
                    for (const auto &local : locals) {
                        account_text(local.second);
                    }
                });
            const auto pc = running.getPC();
            if (pc >= running.getProgram().size()) {
                return;
            }
            const auto &instruction = running.getProgram()[pc];
            const auto &stack = running.getStack();
            const bool console_call = provenance.before(running);
            records.before(running, provenance.dispatched_target(), provenance.current_floor());
            enumerations.before(running, provenance.dispatched_target(), provenance.current_floor());
            for (const auto &global : running.getGlobals()) {
                if (globals.count(global.first) != 1 || !std::holds_alternative<std::int64_t>(global.second)) {
                    throw std::runtime_error("reference global probe: global map name/type drift");
                }
            }
            if (instruction.op == ny::Opcode::STORE_GLOBAL &&
                (stack.empty() || !std::holds_alternative<std::int64_t>(stack.back()))) {
                throw std::runtime_error("reference global probe: STORE_GLOBAL requires i64 value");
            }
            if (instruction.op == ny::Opcode::CAST && instruction.args[0] == "text") {
                const auto *integer = stack.empty() ? nullptr : std::get_if<std::int64_t>(&stack.back());
                if (!integer) {
                    throw std::runtime_error("reference global probe: CAST text requires i64 value");
                }
                const auto size = decimal_bytes(*integer);
                if (size > maximum_computed_text_bytes - computed_text_bytes) {
                    throw std::runtime_error("reference text probe: computed text limit");
                }
                computed_text_bytes += size;
            }
            if (instruction.op == ny::Opcode::NEW_ARRAY_LITERAL) {
                const auto count = *array_literal_count(instruction.args[0]);
                if (array_literals == maximum_array_literals ||
                    count > maximum_array_elements - array_elements) {
                    throw std::runtime_error("reference array probe: array allocation limit");
                }
                if (count > stack.size()) {
                    throw std::runtime_error("reference array probe: literal stack underflow");
                }
                std::size_t text_bytes = 0;
                for (std::size_t index = stack.size() - count; index < stack.size(); ++index) {
                    const auto &value = stack[index];
                    if (instruction.args[1] == "i64") {
                        if (!std::holds_alternative<std::int64_t>(value)) {
                            throw std::runtime_error("reference array probe: literal requires i64 elements");
                        }
                    } else {
                        const auto *text = std::get_if<std::string>(&value);
                        if (!text) {
                            throw std::runtime_error("reference array probe: literal requires text elements");
                        }
                        if (text->size() > maximum_array_text_bytes - array_text_bytes - text_bytes) {
                            throw std::runtime_error("reference array probe: array text limit");
                        }
                        text_bytes += text->size();
                    }
                }
                ++array_literals;
                array_elements += count;
                array_text_bytes += text_bytes;
            } else if (instruction.op == ny::Opcode::LEN) {
                if (stack.empty() || !std::holds_alternative<ny::ArrayHandle>(stack.back())) {
                    throw std::runtime_error("reference array probe: LEN requires an array handle");
                }
            } else if (instruction.op == ny::Opcode::GET_INDEX) {
                if (stack.size() < 2 ||
                    !std::holds_alternative<ny::ArrayHandle>(stack[stack.size() - 2]) ||
                    !std::holds_alternative<std::int64_t>(stack.back())) {
                    throw std::runtime_error("reference array probe: GET_INDEX requires array handle and integer");
                }
            }
            // General VM ADD can allocate concatenated arrays, and other scalar
            // operators can coerce handles. Keep all such producers/consumers
            // outside this immutable array profile and its allocation accounting.
            std::size_t scalar_operands = 0;
            switch (instruction.op) {
            case ny::Opcode::ADD: case ny::Opcode::SUB: case ny::Opcode::MUL:
            case ny::Opcode::DIV: case ny::Opcode::MOD: case ny::Opcode::BIT_AND:
            case ny::Opcode::BIT_OR: case ny::Opcode::BIT_XOR: case ny::Opcode::SHL:
            case ny::Opcode::SHR: case ny::Opcode::CMP_EQ: case ny::Opcode::CMP_NE:
            case ny::Opcode::CMP_LT: case ny::Opcode::CMP_LE: case ny::Opcode::CMP_GT:
            case ny::Opcode::CMP_GE:
                scalar_operands = 2;
                break;
            case ny::Opcode::CAST: case ny::Opcode::NEG: case ny::Opcode::NOT:
            case ny::Opcode::JMP_IF: case ny::Opcode::JMP_IF_NOT:
            case ny::Opcode::CONSOLE_WRITE: case ny::Opcode::CONSOLE_WRITELN:
            case ny::Opcode::INTEROP_CALL:
                scalar_operands = 1;
                break;
            case ny::Opcode::CALL:
                scalar_operands = console_call ? 1 : 0;
                break;
            default:
                break;
            }
            if (scalar_operands <= stack.size()) {
                for (std::size_t index = stack.size() - scalar_operands; index < stack.size(); ++index) {
                    if (std::holds_alternative<ny::ArrayHandle>(stack[index])) {
                        throw std::runtime_error("reference array probe: scalar operation on array handle");
                    }
                }
            }
            if (instruction.op == ny::Opcode::STR_CONCAT) {
                const auto &stack = running.getStack();
                if (stack.size() < 2 ||
                    !std::holds_alternative<std::string>(stack[stack.size() - 2]) ||
                    !std::holds_alternative<std::string>(stack.back())) {
                    throw std::runtime_error("reference text probe: concat requires two text values");
                }
                const auto left = std::get<std::string>(stack[stack.size() - 2]).size();
                const auto right = std::get<std::string>(stack.back()).size();
                const auto remaining = maximum_computed_text_bytes - computed_text_bytes;
                if (left > remaining || right > remaining - left) {
                    throw std::runtime_error("reference text probe: computed text limit");
                }
                computed_text_bytes += left + right;
            }
            const bool pushes = instruction.op == ny::Opcode::PUSH_CONST ||
                                instruction.op == ny::Opcode::PUSH_CONST_STR ||
                                instruction.op == ny::Opcode::PUSH_FN_PTR ||
                                instruction.op == ny::Opcode::NEW_STRUCT ||
                                instruction.op == ny::Opcode::LOAD_GLOBAL ||
                                instruction.op == ny::Opcode::LOAD_LOCAL ||
                                (instruction.op == ny::Opcode::NEW_VARIANT && *enum_count(instruction.args[1]) == 0) ||
                                (instruction.op == ny::Opcode::NEW_ARRAY_LITERAL &&
                                 *array_literal_count(instruction.args[0]) == 0);
            if (pushes && running.getStack().size() == maximum_stack_values) {
                throw std::runtime_error("reference text probe: stack limit");
            }
        });
        vm.loadProgram(program, constants);
        vm.execute();
        if (vm.getStack().size() != 1 ||
            !std::holds_alternative<std::int64_t>(vm.getStack().back())) {
            throw std::runtime_error("reference probe: main did not return one integer");
        }
        result = vm.getProgramExitCode();
    } catch (const std::exception &exception) {
        error = exception.what();
    } catch (...) {
        error = "reference probe: non-standard exception";
    }
    std::cout.imbue(std::locale::classic());
    std::cout << "{\"schema\":1,\"status\":"
              << json_string(error ? "execution_refused" : "returned")
              << ",\"result\":" << (result ? std::to_string(*result) : "null")
              << ",\"error\":" << (error ? json_string(*error) : "null")
              << ",\"output_hex\":" << json_string(hex_bytes(capture.bytes())) << "}\n";
    return std::cout ? 0 : 2;
}
