extern crate std;

use super::*;
use crate::{console_request, service_error};
use anomalous_program_flow::{boolean_control, sha256, validation_workspace};
use anomalous_program_manifest::{authority, maximum_image_bytes};
use std::{boxed::Box, vec, vec::Vec};

const edition: &[u8] = b"\
# NIL Textual Module
module boolean_console
@ai_context(\"Exercise boolean control and acknowledged float output\")
# declaration_line 1
function main -> i64:
PUSH_CONST 0
CAST bool
JMP_IF_NOT false_value
PUSH_CONST 1
CAST bool
STORE_LOCAL selected
JMP join
LABEL false_value
PUSH_CONST 0
STORE_LOCAL selected
LABEL join
LOAD_LOCAL selected
CAST bool
NOT
CAST bool
JMP_IF print_value
PUSH_CONST -1
RET
LABEL print_value
PUSH_CONST 0.5
CALL console__write 1
PUSH_CONST -0.0
CALL println 1
PUSH_CONST 9223372036854775807
RET
end function
";

#[derive(Default)]
struct recorder {
    writes: Vec<Vec<u8>>,
    refuse_sequence: usize,
}

impl console_sink for recorder {
    fn write(&mut self, request: console_request<'_>) -> Result<(), service_error> {
        assert_eq!(request.edition_identity(), sha256(edition));
        assert_eq!(request.sequence(), self.writes.len() + 1);
        if request.sequence() == self.refuse_sequence {
            return Err(service_error::refused);
        }
        self.writes.push(request.bytes().to_vec());
        Ok(())
    }
}

#[test]
fn executes_mixed_local_boolean_control_and_float_console_through_authorized_sink() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let mut validation = Box::new(validation_workspace::new());
            let mut image = vec![0; maximum_image_bytes];
            let length = boolean_control::encode(edition, &mut image, &mut validation).unwrap();
            let image = &image[..length];
            assert!(boolean_control::admit(image, 0, &mut validation).is_err());
            let witness =
                boolean_control::admit(image, authority::console_write, &mut validation).unwrap();
            let mut workspace = Box::new(execution_workspace::new());
            for (budget, refusal, expected, calls, writes) in [
                (6, 0, Ok(i64::MAX), 2, 2),
                (5, 0, Err(error::output_limit), 1, 1),
                (6, 2, Err(error::service), 2, 1),
            ] {
                let mut sink = recorder {
                    refuse_sequence: refusal,
                    ..recorder::default()
                };
                let result = execute(
                    &witness,
                    limits {
                        output_bytes: budget,
                        ..limits::default()
                    },
                    &mut workspace,
                    &mut sink,
                );
                assert_eq!(result.outcome, expected);
                assert_eq!(result.statistics.console_calls, calls);
                assert_eq!(result.statistics.output_bytes, 3 * writes);
                assert_eq!(sink.writes, [b"0.5".as_slice(), b"-0\n"][..writes]);
                assert_eq!(workspace.frame_count, 0);
                assert_eq!(workspace.depth, 0);
                assert!(
                    workspace
                        .frames
                        .iter()
                        .all(|frame| frame.locals.iter().all(Option::is_none))
                );
            }
        })
        .unwrap()
        .join()
        .unwrap();
}
