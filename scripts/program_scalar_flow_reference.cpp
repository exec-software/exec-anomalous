// Independent hosted semantic oracle for bounded profile-3 control-flow editions.
// The qualified NIL checkout is read-only. From the Anomalous repository:
//
// nil_root=/path/to/qualified/nil-context-registration
// c++ -std=c++17 -O2 -I"$nil_root/nilcompiler/src" \
//     scripts/program_scalar_flow_reference.cpp \
//     "$nil_root/build/context-registration/lib/libnilcompiler_core.a" \
//     "$nil_root/build/context-registration/lib/libnil.a" \
//     -ldl -pthread -o build/program_scalar_flow_reference
//
// CLI: program_scalar_flow_reference EDITION_PATH
// One JSON record, with full signed-i64 result and byte-exact output_hex.
// Both semantic return and refusal exit 0. Usage/read failures exit 2 with a
// diagnostic on stderr. Reference exception text is preserved in error.
//
// This is a bounded test host, not a general NIL sandbox or an admission
// implementation. It calls the qualified VM rather than duplicating execution
// semantics. Whole-edition candidate admission is a separate operation. The
// effect guard accepts direct typed calls and function-local branches, including
// known instructions in dead blocks and the generated fallthrough RET marker.
// It permits no indirect calls, imports, native calls, input, filesystem,
// environment, AI or UI effects. The only intrinsic forms are console output.
// The fresh VM has no FFI registrations for console's hot-reload hook to visit.
// Profiling is disabled before VM setup. Main is entered directly; generated
// module function-skip scaffolding is never dispatched.
//
// Static bounds: 64 KiB edition, 4096 body instructions, 64 functions, 128
// constants, 128 locals per function, 8 parameters. Main is zero-argument i32
// or i64; helpers may use i32/i64/text parameters and i32/i64/text/void returns.
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
// Profile 3 is a scalar subrestriction of the qualified 4dad051 VM. Generated
// locals are accepted only as LOAD_LOCAL/STORE_LOCAL operands with spelling
// $ny.temp.<simple ASCII identifier>.<canonical decimal>, at most 128 bytes.
// Dotted compiler prefixes (for example query.source) remain outside this
// profile. Function, constant, label and CALL names remain ordinary identifiers.
// Pinned NIL source: codegen.cpp:1220-1226 and 6114-6119 generate per-function
// scratch names; 2202-2246 and 2256-2325 lower range loops with operandless ADD.
// vm.cpp:290-368 selects i64 arithmetic for two integer values and wraps its
// result; 6766-6792 also supports floats/arrays in the general VM. This guard
// permits neither producer, so bare ADD retains exact scalar i64 semantics
// for integers and the VM's own refusal for text operands. No instruction is
// rewritten, and all prior reference sources remain independently reproducible.

#include "nil_loader.hpp"
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

bool value_type(const std::string &type) {
    return integer_width(type) || type == "text";
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

bool allowed_body_instruction(const ny::Instruction &instruction) {
    using ny::Opcode;
    const auto &args = instruction.args;
    if (instruction.is_function_entry || instruction.is_function_end) {
        return false;
    }
    switch (instruction.op) {
    case Opcode::PUSH_CONST:
        return args.size() == 1 && signed_decimal(args[0]);
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
    case Opcode::NOT:
    case Opcode::POP:
    case Opcode::NOP:
    case Opcode::CONSOLE_WRITE:
    case Opcode::CONSOLE_WRITELN:
        return args.empty();
    case Opcode::RET:
        return args.empty() || (args.size() == 1 && args[0] == fallthrough_operand);
    case Opcode::CALL:
        return args.size() == 2 && safe_name(args[0]) && argument_count(args[1]).has_value();
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
};

void guard_effects(const std::vector<ny::Instruction> &program,
                  const std::unordered_map<std::string, ny::Value> &constants) {
    using ny::Opcode;
    const auto refuse = []() {
        throw std::runtime_error("reference scalar flow probe: outside profile-3 effect guard");
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
            !(value_type(entry.function_return_type) || entry.function_return_type == "void") ||
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
            if (!value_type(type)) {
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
        functions.push_back({entry.args[0], entry.function_param_count, begin, end_pc});
    }
    if (arities.count("main") != 1) {
        refuse();
    }
    for (const auto &function : functions) {
        std::unordered_set<std::string> local_labels;
        std::unordered_set<std::string> locals;
        for (std::size_t index = function.begin; index < function.end; ++index) {
            const auto &instruction = program[index];
            if (instruction.op == Opcode::LABEL) {
                local_labels.insert(instruction.args[0]);
            }
            if (instruction.op == Opcode::LOAD_LOCAL || instruction.op == Opcode::STORE_LOCAL) {
                const auto &name = instruction.args[0];
                // A local text value otherwise redirects CALL in the VM.
                if (arities.count(name) != 0 || console_builtin(name)) {
                    refuse();
                }
                locals.insert(name);
                if (locals.size() > 128) {
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
                if (!count || (console_builtin(args[0]) ? *count != 1 :
                    declared == arities.end() || declared->second != *count)) {
                    refuse();
                }
            } else if (instruction.op == Opcode::PUSH_CONST_STR && constants.count(args[0]) != 1) {
                refuse();
            }
        }
    }
}

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
            // The qualified VM's profile-3 diagnostics are ASCII. Escaping
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
        std::cerr << "usage: program_scalar_flow_reference EDITION_PATH\n";
        return 2;
    }
    std::string edition;
    try {
        edition = read_edition(argv[1]);
        if (::unsetenv("NIL_MEMORY_PROFILE") != 0) {
            throw std::runtime_error("cannot disable memory profiling");
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
        guard_effects(program, constants);
        ny::VM vm;
        vm.setOutputStream(output);
        vm.setProcessExitTrapping(true);
        std::size_t steps = 0;
        vm.setControlCallback([&steps](ny::VM &running) {
            if (steps == maximum_steps) {
                throw std::runtime_error("reference scalar flow probe: instruction limit");
            }
            ++steps;
            if (running.getCallDepth() > maximum_frames ||
                running.getStack().size() > maximum_stack_values) {
                throw std::runtime_error("reference scalar flow probe: frame/stack limit");
            }
            const auto pc = running.getPC();
            if (pc >= running.getProgram().size()) {
                return;
            }
            const auto &instruction = running.getProgram()[pc];
            if (instruction.op == ny::Opcode::CALL && running.hasFunction(instruction.args[0]) &&
                running.getCallDepth() == maximum_frames) {
                throw std::runtime_error("reference scalar flow probe: frame limit");
            }
            const bool pushes = instruction.op == ny::Opcode::PUSH_CONST ||
                                instruction.op == ny::Opcode::PUSH_CONST_STR ||
                                instruction.op == ny::Opcode::LOAD_LOCAL;
            if (pushes && running.getStack().size() == maximum_stack_values) {
                throw std::runtime_error("reference scalar flow probe: stack limit");
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
