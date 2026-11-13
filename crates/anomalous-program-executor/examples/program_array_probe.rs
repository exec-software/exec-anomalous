//! Host-only profile-5 typed-array probe; semantic results use the original probe JSON shape.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]
use anomalous_program_executor::{console_request, console_sink, flow, service_error};
use anomalous_program_flow::{
    array::{admit, encode},
    validation_workspace,
};
use anomalous_program_manifest::{maximum_edition_bytes, maximum_image_bytes};
use std::io::Read;

#[derive(Default)]
struct recorder(Vec<u8>);
impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.0.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn refused(error: anomalous_program_manifest::error) {
    println!(
        "{{\"status\":\"admission_refused\",\"result\":null,\"error\":\"{}\",\"output_hex\":\"\",\"instructions\":0,\"console_calls\":0,\"output_bytes\":0}}",
        error.as_str()
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if !(3..=5).contains(&arguments.len()) {
        return Err("usage: program_array_probe <edition> <ceiling> [steps] [frames]".into());
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
    let mut image = vec![0; maximum_image_bytes];
    let mut validation = Box::new(validation_workspace::new());
    let length = match encode(&edition, &mut image, &mut validation) {
        Ok(length) => length,
        Err(error) => {
            refused(error);
            return Ok(());
        }
    };
    let admitted = match admit(&image[..length], ceiling, &mut validation) {
        Ok(admitted) => admitted,
        Err(error) => {
            refused(error);
            return Ok(());
        }
    };
    let mut execution = Box::new(flow::execution_workspace::new());
    let mut output = recorder::default();
    let result = flow::execute(&admitted, budget, &mut execution, &mut output);
    let (status, returned, error) = match result.outcome {
        Ok(value) => ("returned", value.to_string(), "null".to_owned()),
        Err(error) => (
            "execution_refused",
            "null".to_owned(),
            format!("\"{}\"", error.as_str()),
        ),
    };
    let output_hex = output
        .0
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    println!(
        "{{\"status\":\"{status}\",\"result\":{returned},\"error\":{error},\"output_hex\":\"{output_hex}\",\"instructions\":{},\"console_calls\":{},\"output_bytes\":{}}}",
        result.statistics.instructions,
        result.statistics.console_calls,
        result.statistics.output_bytes
    );
    Ok(())
}
