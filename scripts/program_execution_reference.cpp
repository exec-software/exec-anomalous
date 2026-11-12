// Independent hosted semantic oracle for already-admitted profile-1 editions.
// The qualified NIL checkout is read-only. From the Anomalous repository:
//
// nil_root=/path/to/qualified/nil-context-registration
// c++ -std=c++17 -O2 -I"$nil_root/nilcompiler/src" \
//     scripts/program_execution_reference.cpp \
//     "$nil_root/build/context-registration/lib/libnilcompiler_core.a" \
//     "$nil_root/build/context-registration/lib/libnil.a" \
//     -ldl -pthread -o build/program_execution_reference
//
// CLI: program_execution_reference EDITION_PATH
// One JSON record, with full signed-i64 result and byte-exact output_hex.
// Both semantic return and refusal exit 0. Usage/read failures exit 2 with a
// diagnostic on stderr. Reference exception text is preserved in error.
//
// This is a bounded test host, not a general NIL sandbox or an admission
// implementation. Its effect guard checks parsed instruction shapes before
// invoking the qualified VM. The caller must separately admit the same bytes
// under Anomalous manifest profile 1. The guard permits no imports, native
// calls, user calls, branches, input, filesystem, environment, AI or UI effects.
// Parser-generated function skips are validated but never run: main is entered
// directly and its body must end in RET. Profiling is disabled before VM setup.
// The fresh VM has no FFI registrations for console's hot-reload hook to visit.

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

bool allowed_body_instruction(const ny::Instruction &instruction) {
    using ny::Opcode;
    const auto &args = instruction.args;
    if (instruction.is_function_entry || instruction.is_function_end) {
        return false;
    }
    switch (instruction.op) {
    case Opcode::PUSH_CONST:
        return args.size() == 1 && signed_decimal(args[0]);
    case Opcode::PUSH_CONST_STR:
    case Opcode::LOAD_LOCAL:
    case Opcode::STORE_LOCAL:
        return args.size() == 1;
    case Opcode::CAST:
    case Opcode::NEG:
    case Opcode::ADD:
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
    case Opcode::RET:
    case Opcode::CONSOLE_WRITE:
    case Opcode::CONSOLE_WRITELN:
        return args.empty();
    case Opcode::CALL:
        return args.size() == 2 && console_builtin(args[0]) && args[1] == "1";
    case Opcode::INTEROP_CALL:
        return args.size() == 4 && args[0] == "intrinsic" &&
               (args[1] == "console.write" || args[1] == "console.writeln") &&
               args[2] == "1" && args[3] == "void";
    default:
        return false;
    }
}

void guard_effects(const std::vector<ny::Instruction> &program,
                  const std::unordered_map<std::string, ny::Value> &constants) {
    using ny::Opcode;
    const auto refuse = []() {
        throw std::runtime_error("reference probe: outside profile-1 effect guard");
    };
    if (constants.size() > 128 || program.size() > maximum_instructions + 64 * 3) {
        refuse();
    }
    for (const auto &constant : constants) {
        if (!std::holds_alternative<std::string>(constant.second)) {
            refuse();
        }
    }
    std::unordered_set<std::string> names;
    std::size_t instructions = 0;
    std::size_t pc = 0;
    while (pc < program.size()) {
        const auto skip_label = "$ny.internal.function-end." + std::to_string(names.size());
        const auto &skip = program[pc++];
        if (skip.op != Opcode::JMP || skip.args != std::vector<std::string>{skip_label} ||
            skip.is_function_entry || skip.is_function_end || pc == program.size()) {
            refuse();
        }
        const auto &entry = program[pc++];
        if (entry.op != Opcode::LABEL || !entry.is_function_entry ||
            entry.is_function_end || entry.args.size() != 1 ||
            entry.function_param_count != 0 || !entry.function_parameter_types.empty() ||
            !integer_width(entry.function_return_type) || console_builtin(entry.args[0]) ||
            !names.insert(entry.args[0]).second || names.size() > 64) {
            refuse();
        }
        bool returned = false;
        while (pc < program.size() && !program[pc].is_function_end) {
            const auto &instruction = program[pc++];
            if (returned || !allowed_body_instruction(instruction) ||
                ++instructions > maximum_instructions) {
                refuse();
            }
            returned = instruction.op == Opcode::RET;
        }
        if (!returned || pc == program.size()) {
            refuse();
        }
        const auto &end = program[pc++];
        if (end.op != Opcode::LABEL || end.is_function_entry ||
            end.args != std::vector<std::string>{skip_label}) {
            refuse();
        }
    }
    if (names.count("main") != 1) {
        refuse();
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
            // The qualified VM's profile-1 diagnostics are ASCII. Escaping
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
        std::cerr << "usage: program_execution_reference EDITION_PATH\n";
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
            if (++steps > maximum_instructions + 1 || running.getStack().size() > 256) {
                throw std::runtime_error("reference probe: instruction/stack limit");
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
