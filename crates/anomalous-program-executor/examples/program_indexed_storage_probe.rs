//! Run the profile 14 production admission path on a host edition.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]

use anomalous_program_executor::flow;
use anomalous_program_flow::{indexed_storage, validation_workspace};
use anomalous_program_manifest::maximum_image_bytes;

#[path = "support/flow_receipt.rs"]
mod probe;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (edition, ceiling, budget) = probe::input()?;
    let mut validation = Box::new(validation_workspace::new());
    let mut image = vec![0; maximum_image_bytes];
    let admitted = match indexed_storage::encode(&edition, &mut image, &mut validation) {
        Ok(length) => indexed_storage::admit(&image[..length], ceiling, &mut validation),
        Err(error) => Err(error),
    };
    let admitted = match admitted {
        Ok(admitted) => admitted,
        Err(error) => {
            probe::refused("admission_refused", error);
            return Ok(());
        }
    };
    let mut workspace = Box::new(flow::execution_workspace::new());
    let mut output = probe::recorder::default();
    let result = flow::execute(&admitted, budget, &mut workspace, &mut output);
    probe::report(admitted.program(), result, &output);
    Ok(())
}
