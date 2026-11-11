#![no_std]
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals)]

//! Experimental native startup data and a private, precommitted byte arena.
//! Descriptor values describe a launch; they cannot grant authority or map RAM.
//! The allocator only splits exclusively borrowed storage. It has no reset,
//! deallocation, growth, global allocator, or operating-system dependency.

#[path = "arena.rs"]
mod allocation;
mod console;
mod program_startup;
mod startup;

pub use allocation::{arena, arena_error};
pub use console::{
    console_error, console_transport, console_writer, maximum_write_bytes, validate_console_text,
};
pub use program_startup::{
    granted_handle, maximum_arena_bytes, maximum_edition_bytes, minimum_arena_bytes,
    program_descriptor, program_startup_error, program_startup_size, service,
};
pub use startup::{arena_byte_count, startup_descriptor, startup_error, startup_size};

/// Experimental ANPG register-gate results. These preserve the native packet
/// gate's numeric conventions; they are not the separate `rite_status` wire
/// vocabulary and do not stabilize a general application ABI.
pub mod program_status {
    pub const ok: u64 = 0;
    pub const denied: u64 = 2;
    pub const bad_address: u64 = 3;
    pub const invalid: u64 = 4;
    pub const closed: u64 = 5;
}
