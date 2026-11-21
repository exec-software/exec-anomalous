//! Exact binary64 literal observations from the immutable validated plan.
use anomalous_program_flow::{operation, program};
use std::fmt::Write;

pub fn snapshot(program: &program<'_>) -> String {
    let mut output = String::from("[");
    let mut separator = "";
    for function in program.functions() {
        for index in function.start..function.end {
            let operation::float(bits) = program.instructions()[index].operation else {
                continue;
            };
            write!(
                output,
                "{separator}{{\"function\":\"{}\",\"instruction_index\":{},\"bits_hex\":\"{bits:016x}\"}}",
                function.name,
                index - function.start
            )
            .expect("write to String");
            separator = ",";
        }
    }
    output.push(']');
    output
}
