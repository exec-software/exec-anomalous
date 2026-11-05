//! Opt-in Linux x86-64 arch_prctl policy. The native owner authenticates the
//! pending caller and stages effects; only this userspace module assigns Linux
//! option numbers and errno meanings. Bases are values, not mapping authority.

use crate::{
    completion, completion_action, invocation, linux_policy, native_status, stage_kind,
    stage_receipt,
};

const native_user_limit: u64 = 1 << 47;
// Linux v6.17 task_size_max(), four-level paging, reserves the top canonical
// page. This userspace policy limit is narrower than native FS ownership.
const linux_task_size_max: u64 = native_user_limit - 4096;
const arch_set_fs: i32 = 0x1002;
const arch_get_fs: i32 = 0x1003;

pub trait backend: linux_policy::backend {
    fn caller_fs_query(&mut self) -> Result<u64, native_status>;
    fn caller_fs_stage(&mut self, base: u64) -> Result<stage_receipt, native_status>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum policy_error {
    legacy(linux_policy::policy_error),
    invalid_stage_receipt,
    invalid_fs_observation,
}

pub fn evaluate(
    request: invocation,
    backend: &mut impl backend,
) -> Result<completion, policy_error> {
    if request.number() as u32 as i32 != 158 {
        return linux_policy::evaluate(request, backend).map_err(policy_error::legacy);
    }
    let arguments = request.arguments();
    match arguments[0] as u32 as i32 {
        arch_set_fs => {
            // This qualified lower-48-bit profile follows Linux's EPERM for a
            // base outside TASK_SIZE_MAX. Zero and unmapped user bases are valid.
            if arguments[1] >= linux_task_size_max {
                return Ok(completion::returned(-1));
            }
            match backend.caller_fs_stage(arguments[1]) {
                Ok(receipt) => staged(request.sequence(), receipt, 0),
                Err(native_status::invalid) => Ok(completion::returned(-1)),
                Err(error) => Ok(completion::returned(native_error(error))),
            }
        }
        arch_get_fs => {
            let base = match backend.caller_fs_query() {
                Ok(base) if base < native_user_limit => base,
                Ok(_) => return Err(policy_error::invalid_fs_observation),
                Err(error) => return Ok(completion::returned(native_error(error))),
            };
            let bytes = base.to_le_bytes();
            match backend.stage(stage_kind::copyout, arguments[1], 8, &bytes) {
                Ok(receipt) => staged(request.sequence(), receipt, 8),
                Err(error) => Ok(completion::returned(native_error(error))),
            }
        }
        // GS and other arch_prctl families are explicitly unsupported here.
        _ => Ok(completion::returned(-22)),
    }
}

fn staged(sequence: u64, receipt: stage_receipt, length: u64) -> Result<completion, policy_error> {
    if receipt.token != sequence || receipt.length != length {
        return Err(policy_error::invalid_stage_receipt);
    }
    Ok(completion {
        result_bits: 0,
        action: completion_action::return_to_caller,
        stage_token: receipt.token,
    })
}

fn native_error(status: native_status) -> i64 {
    match status {
        native_status::denied => -1,
        native_status::not_ready => -11,
        native_status::bad_copy => -14,
        native_status::closed => -19,
        native_status::invalid | native_status::ok => -22,
        native_status::too_small => -90,
        native_status::exhausted => -75,
    }
}

#[cfg(test)]
mod tests;
