//! Run the opt-in profile 13 raw validation path on a host edition.
#![forbid(unsafe_code)]
#![allow(non_camel_case_types)]

use anomalous_program_executor::{conformance, flow};
use anomalous_program_flow::{conformance::validate_hygienic_locals, validation_workspace};

#[path = "support/hygienic_locals.rs"]
mod probe;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (edition, ceiling, budget) = probe::input()?;
    let mut validation = Box::new(validation_workspace::new());
    let checked = match validate_hygienic_locals(&edition, ceiling, &mut validation) {
        Ok(checked) => checked,
        Err(error) => {
            probe::refused("validation_refused", error);
            return Ok(());
        }
    };
    let mut workspace = Box::new(flow::execution_workspace::new());
    let mut output = probe::recorder::default();
    let result = conformance::execute(&checked, budget, &mut workspace, &mut output);
    probe::report(checked.program(), result, &output);
    Ok(())
}
