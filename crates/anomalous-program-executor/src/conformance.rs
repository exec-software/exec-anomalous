//! Opt-in host-only semantic experiments over complete, unchanged raw editions.
//! This entry cannot produce or substitute for production manifest admission.

use crate::{console_sink, execution, flow};
use anomalous_program_flow::conformance::checked_edition;

/// Execute a raw scalar, text, array, callable or record validation witness
/// with the same bounded dispatch and explicit console service as production-
/// admitted host experiments. Absence of
/// authored intent is allowed only by this separate validation/execution route.
/// This API grants no native task authority and is unavailable for freestanding
/// targets. A raw witness cannot be passed to the production entry:
///
/// ```compile_fail
/// use anomalous_program_executor::{console_sink, flow};
/// use anomalous_program_flow::conformance::checked_edition;
/// fn cannot_admit<'a>(checked: &checked_edition<'a, '_>,
///     workspace: &mut flow::execution_workspace<'a>, sink: &mut impl console_sink) {
///     flow::execute(checked, flow::limits::default(), workspace, sink);
/// }
/// ```
pub fn execute<'a>(
    checked: &checked_edition<'a, '_>,
    budget: flow::limits,
    workspace: &mut flow::execution_workspace<'a>,
    sink: &mut impl console_sink,
) -> execution {
    flow::execute_checked(
        checked.program(),
        checked.identity(),
        checked.granted_services(),
        budget,
        workspace,
        sink,
    )
}
