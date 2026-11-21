//! Read-only host receipt of already validated metadata; never opens a source path.
use anomalous_program_flow::program;
use std::fmt::Write;

fn hex(bytes: impl Iterator<Item = u8>) -> String {
    let mut output = String::new();
    for byte in bytes {
        write!(output, "{byte:02x}").expect("write to String");
    }
    output
}

pub fn snapshot(program: &program<'_>) -> String {
    let mut output = String::from("[");
    let mut separator = "";
    for function in program.functions() {
        for index in function.start..function.end {
            let Some(record) = program.source_debug(index) else {
                continue;
            };
            // Validated function names contain only the admitted ASCII vocabulary.
            write!(
                output,
                "{separator}{{\"function\":\"{}\",\"instruction_index\":{},\"path_hex\":\"{}\",\"bindings\":[",
                function.name,
                record.instruction_index(),
                hex(record.path().bytes())
            )
            .expect("write to String");
            separator = ",";
            let mut binding_separator = "";
            for binding in record.bindings() {
                write!(
                    output,
                    "{binding_separator}{{\"name_hex\":\"{}\",\"storage_hex\":\"{}\",\"type_hex\":\"{}\",\"scope_depth\":{}}}",
                    hex(binding.name.bytes()),
                    hex(binding.storage.bytes()),
                    hex(binding.lexical_type.bytes()),
                    binding.scope_depth
                )
                .expect("write to String");
                binding_separator = ",";
            }
            output.push_str("]}");
        }
    }
    output.push(']');
    output
}
