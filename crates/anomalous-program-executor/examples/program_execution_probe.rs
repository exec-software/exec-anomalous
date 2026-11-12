//! Host-only JSON probe. Not linked into any vessel or kernel image.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]
use anomalous_program_executor::{console_request, console_sink, execute, limits, service_error};
use anomalous_program_manifest::{admit, encode, maximum_edition_bytes, maximum_image_bytes};
use std::io::Read;

#[derive(Default)]
struct recorder(Vec<u8>);
impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        self.0.extend_from_slice(request.bytes());
        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().collect();
    if arguments.len() != 3 {
        return Err("usage: program_execution_probe <edition> <ceiling>".into());
    }
    let ceiling: u64 = arguments[2].to_str().ok_or("ceiling encoding")?.parse()?;
    let mut edition = Vec::new();
    std::fs::File::open(&arguments[1])?
        .take(maximum_edition_bytes as u64 + 1)
        .read_to_end(&mut edition)?;
    let mut image = vec![0; maximum_image_bytes];
    let admitted = encode(&edition, &mut image).and_then(|length| admit(&image[..length], ceiling));
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(error) => {
            println!(
                "{{\"status\":\"admission_refused\",\"result\":null,\"error\":\"{}\",\"output_hex\":\"\",\"instructions\":0,\"console_calls\":0,\"output_bytes\":0}}",
                error.as_str()
            );
            return Ok(());
        }
    };
    let mut output = recorder::default();
    let result = execute(&admitted, limits::default(), &mut output);
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
