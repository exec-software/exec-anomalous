#![allow(dead_code)]

pub const page_size: u64 = 4096;
pub const user_image_address: u64 = 0x0000_6000_0000_1000;
pub const user_image_page_limit: u64 = 64;
pub const user_mailbox_address: u64 = 0x0000_6000_0008_0000;
pub const user_stack_address: u64 = 0x0000_6000_0009_0000;
pub const user_stack_page_count: u64 = 4;
pub const user_stack_top: u64 = user_stack_address + user_stack_page_count * page_size;
pub const rite_gate_vector: u8 = 0xf3;
pub const initial_user_flags: u64 = 0x0002;

const _: () = {
    assert!(user_image_address + user_image_page_limit * page_size < user_mailbox_address);
    assert!(user_mailbox_address + page_size < user_stack_address);
    assert!(user_stack_top < 0x0000_8000_0000_0000);
};
