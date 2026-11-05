#![allow(dead_code)]

pub const page_size: u64 = 4096;
pub const user_image_address: u64 = 0x0000_6000_0000_1000;
pub const user_image_page_limit: u64 = 64;
pub const generation_2_entry_address: u64 = user_image_address + 0x200;
pub const user_mailbox_address: u64 = 0x0000_6000_0008_0000;
pub const user_stack_address: u64 = 0x0000_6000_0009_0000;
pub const user_stack_page_count: u64 = 4;
pub const user_stack_top: u64 = user_stack_address + user_stack_page_count * page_size;

pub const scheduler_gate_vector: u8 = 0xf2;
pub const rite_gate_vector: u8 = 0xf3;
pub const scheduler_ready_operation: u64 = 1;
pub const initial_user_flags: u64 = 0x0002;
pub const running_user_flags: u64 = 0x0202;

pub const mailbox_canary_a_offset: u64 = 0;
pub const mailbox_canary_b_offset: u64 = 8;
pub const mailbox_arm_offset: u64 = 16;
pub const generation_1_canary_a: u64 = 0x72d1_5ca8_4e30_b691;
pub const generation_1_canary_b: u64 = 0xc438_9fe2_17a6_5bd0;
pub const generation_1_arm: u64 = 0xa671_3dc9_e254_8b0f;

const _: () = {
    assert!(user_image_address + user_image_page_limit * page_size < user_mailbox_address);
    assert!(user_mailbox_address + page_size < user_stack_address);
    assert!(user_stack_top < 0x0000_8000_0000_0000);
};
