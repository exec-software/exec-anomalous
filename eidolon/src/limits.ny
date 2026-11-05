module limits

# Eidolon's portable Ny profile uses range-checked i32 logical octets. A
# logical byte currently occupies much more than one resident byte in the
# interpreter, and whole-buffer transforms create transient copies. These are
# therefore conservative acceptance limits shared by every public codec layer,
# not wire-format maxima and not a claim about process RSS.
const max_width_value: i32 = 4096
const max_height_value: i32 = 4096
const max_tile_edge_value: i32 = 256
const max_frames_value: i32 = 4096
const max_tiles_per_frame_value: i32 = 1048576
const max_frame_bytes_value: i32 = 1048576
const max_payload_bytes_value: i32 = 1048576
const max_container_bytes_value: i32 = 1048576
const max_total_decoded_bytes_value: i32 = 1048576

# Cross-module constant loads are not yet an interpreter contract. Public
# accessors make this module the one executable source of truth.
pub fn maximum_width() -> i32
    return max_width_value
end fn

pub fn maximum_height() -> i32
    return max_height_value
end fn

pub fn maximum_tile_edge() -> i32
    return max_tile_edge_value
end fn

pub fn maximum_frames() -> i32
    return max_frames_value
end fn

pub fn maximum_tiles_per_frame() -> i32
    return max_tiles_per_frame_value
end fn

pub fn maximum_frame_bytes() -> i32
    return max_frame_bytes_value
end fn

pub fn maximum_payload_bytes() -> i32
    return max_payload_bytes_value
end fn

pub fn maximum_container_bytes() -> i32
    return max_container_bytes_value
end fn

pub fn maximum_total_decoded_bytes() -> i32
    return max_total_decoded_bytes_value
end fn
