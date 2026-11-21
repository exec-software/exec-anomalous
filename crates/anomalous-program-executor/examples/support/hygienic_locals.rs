use anomalous_program_executor::{console_request, console_sink, execution, flow, service_error};
use anomalous_program_flow::program;
use anomalous_program_manifest::{error, maximum_edition_bytes};
use std::{fmt::Write, io::Read};

#[path = "source_debug.rs"]
mod source_debug;
#[path = "typed_scalars.rs"]
mod typed_scalars;

#[derive(Default)]
pub(super) struct recorder(Vec<u8>);

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.0.extend_from_slice(request.bytes());
        Ok(())
    }
}

pub(super) fn input() -> Result<(Vec<u8>, u64, flow::limits), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if !(3..=5).contains(&arguments.len()) {
        return Err("usage: <probe> <edition> <ceiling> [steps] [frames]".into());
    }
    let ceiling = arguments[2].to_str().ok_or("ceiling encoding")?.parse()?;
    let mut budget = flow::limits::default();
    if let Some(steps) = arguments.get(3) {
        budget.instructions = steps.to_str().ok_or("steps encoding")?.parse()?;
    }
    if let Some(frames) = arguments.get(4) {
        budget.frames = frames.to_str().ok_or("frames encoding")?.parse()?;
    }
    let mut edition = Vec::new();
    std::fs::File::open(&arguments[1])?
        .take(maximum_edition_bytes as u64 + 1)
        .read_to_end(&mut edition)?;
    Ok((edition, ceiling, budget))
}

pub(super) fn refused(status: &str, error: error) {
    println!(
        "{{\"status\":\"{status}\",\"result\":null,\"error\":\"{}\",\"output_hex\":\"\",\"instructions\":0,\"console_calls\":0,\"output_bytes\":0,\"source_debug\":[],\"f64_literals\":[]}}",
        error.as_str()
    );
}

pub(super) fn report(program: &program<'_>, result: execution, output: &recorder) {
    let metadata = source_debug::snapshot(program);
    let f64_literals = typed_scalars::snapshot(program);
    let (status, returned, error) = match result.outcome {
        Ok(value) => ("returned", value.to_string(), "null".to_owned()),
        Err(error) => (
            "execution_refused",
            "null".to_owned(),
            format!("\"{}\"", error.as_str()),
        ),
    };
    let mut output_hex = String::with_capacity(output.0.len() * 2);
    for byte in &output.0 {
        write!(output_hex, "{byte:02x}").expect("write to String");
    }
    println!(
        "{{\"status\":\"{status}\",\"result\":{returned},\"error\":{error},\"output_hex\":\"{output_hex}\",\"instructions\":{},\"console_calls\":{},\"output_bytes\":{},\"source_debug\":{metadata},\"f64_literals\":{f64_literals}}}",
        result.statistics.instructions,
        result.statistics.console_calls,
        result.statistics.output_bytes
    );
}
