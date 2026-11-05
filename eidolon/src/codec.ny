module codec

import limits

# This first production-shaped slice deliberately represents an octet as an
# i32 constrained to 0..255. The current route proves byte values and draft-1
# layout in memory; it does not claim a native u8 layout or file ABI.

pub enum tile_mode
    raw
    solid
    pixel_rle
    unchanged
    xor_rle
end enum

pub enum codec_status
    ok
    invalid_geometry
    invalid_frame_length
    invalid_octet
    truncated
    unknown_mode
    invalid_tile_length
    zero_run
    run_overflow
    underfill
    temporal_in_key
    missing_previous
    trailing_data
    resource_limit
end enum

struct tile_shape
    x: i32
    y: i32
    width: i32
    height: i32
    byte_count: i32
end struct

struct tile_choice
    mode: tile_mode
    payload_size: i32
end struct

pub struct encoded_payload
    # A result always owns both pointers, including failures. Call release_encoded.
    pub data: ptr<i32>
    pub length: i32
    pub modes: ptr<i32>
    pub mode_count: i32
    pub status: codec_status
end struct

pub struct decoded_frame
    # Pixels become observable only when status is ok. Call release_decoded.
    pub pixels: ptr<i32>
    pub length: i32
    pub modes: ptr<i32>
    pub mode_count: i32
    pub status: codec_status
end struct

pub fn status_code(status: codec_status) -> i32
    match status
        codec_status.ok -> return 0
        codec_status.invalid_geometry -> return 1
        codec_status.invalid_frame_length -> return 2
        codec_status.invalid_octet -> return 3
        codec_status.truncated -> return 4
        codec_status.unknown_mode -> return 5
        codec_status.invalid_tile_length -> return 6
        codec_status.zero_run -> return 7
        codec_status.run_overflow -> return 8
        codec_status.underfill -> return 9
        codec_status.temporal_in_key -> return 10
        codec_status.missing_previous -> return 11
        codec_status.trailing_data -> return 12
        codec_status.resource_limit -> return 13
    end match
    return 255
end fn

fn mode_code(mode: tile_mode) -> i32
    match mode
        tile_mode.raw -> return 0
        tile_mode.solid -> return 1
        tile_mode.pixel_rle -> return 2
        tile_mode.unchanged -> return 3
        tile_mode.xor_rle -> return 4
    end match
    return 255
end fn

fn minimum(left: i32, right: i32) -> i32
    if left < right then
        return left
    end if
    return right
end fn

fn octet_is_valid(value: i32) -> bool
    return value >= 0 && value <= 255
end fn

fn array_contains_only_octets(values: array<i32>) -> bool
    let mut index: i32 = 0
    while index < values.len()
        if not octet_is_valid(values[index]) then
            return false
        end if
        index = index + 1
    end while
    return true
end fn

fn geometry_is_valid(width: i32, height: i32, tile_width: i32, tile_height: i32) -> bool
    if width < 1 || height < 1 || tile_width < 1 || tile_height < 1 then
        return false
    end if
    if width > limits::maximum_width() || height > limits::maximum_height() then
        return false
    end if
    if tile_width > limits::maximum_tile_edge() ||
        tile_height > limits::maximum_tile_edge() then
        return false
    end if
    let columns = (width + tile_width - 1) / tile_width
    let rows = (height + tile_height - 1) / tile_height
    if rows > limits::maximum_tiles_per_frame() / columns then
        return false
    end if
    return true
end fn

fn frame_size(width: i32, height: i32) -> i32
    return width * height * 3
end fn

fn tile_columns(width: i32, tile_width: i32) -> i32
    return (width + tile_width - 1) / tile_width
end fn

fn tile_rows(height: i32, tile_height: i32) -> i32
    return (height + tile_height - 1) / tile_height
end fn

fn shape_for_tile(
    width: i32,
    height: i32,
    tile_width: i32,
    tile_height: i32,
    tile_number: i32
) -> tile_shape
    let columns = tile_columns(width, tile_width)
    let tile_x = tile_number % columns
    let tile_y = tile_number / columns
    let origin_x = tile_x * tile_width
    let origin_y = tile_y * tile_height
    let actual_width = minimum(tile_width, width - origin_x)
    let actual_height = minimum(tile_height, height - origin_y)
    return tile_shape {
        x: origin_x,
        y: origin_y,
        width: actual_width,
        height: actual_height,
        byte_count: actual_width * actual_height * 3
    }
end fn

fn frame_index_for_tile_byte(shape: tile_shape, frame_width: i32, byte_offset: i32) -> i32
    let pixel_offset = byte_offset / 3
    let component = byte_offset % 3
    let local_x = pixel_offset % shape.width
    let local_y = pixel_offset / shape.width
    return ((shape.y + local_y) * frame_width + shape.x + local_x) * 3 + component
end fn

fn tile_value(frame: array<i32>, shape: tile_shape, frame_width: i32, byte_offset: i32) -> i32
    return frame[frame_index_for_tile_byte(shape, frame_width, byte_offset)]
end fn

fn same_pixel(
    frame: array<i32>,
    shape: tile_shape,
    frame_width: i32,
    left_pixel: i32,
    right_pixel: i32
) -> bool
    let left = left_pixel * 3
    let right = right_pixel * 3
    return tile_value(frame, shape, frame_width, left) == tile_value(frame, shape, frame_width, right) &&
        tile_value(frame, shape, frame_width, left + 1) == tile_value(frame, shape, frame_width, right + 1) &&
        tile_value(frame, shape, frame_width, left + 2) == tile_value(frame, shape, frame_width, right + 2)
end fn

fn tile_is_solid(frame: array<i32>, shape: tile_shape, frame_width: i32) -> bool
    let pixel_count = shape.byte_count / 3
    let mut pixel: i32 = 1
    while pixel < pixel_count
        if not same_pixel(frame, shape, frame_width, 0, pixel) then
            return false
        end if
        pixel = pixel + 1
    end while
    return true
end fn

fn tile_matches_previous(
    frame: array<i32>,
    previous: array<i32>,
    shape: tile_shape,
    frame_width: i32
) -> bool
    let mut offset: i32 = 0
    while offset < shape.byte_count
        let source_index = frame_index_for_tile_byte(shape, frame_width, offset)
        if frame[source_index] != previous[source_index] then
            return false
        end if
        offset = offset + 1
    end while
    return true
end fn

fn pixel_run_count(frame: array<i32>, shape: tile_shape, frame_width: i32) -> i32
    let pixel_count = shape.byte_count / 3
    let mut runs: i32 = 0
    let mut pixel: i32 = 0
    while pixel < pixel_count
        let mut run: i32 = 1
        while pixel + run < pixel_count && run < 65535 && same_pixel(frame, shape, frame_width, pixel, pixel + run)
            run = run + 1
        end while
        runs = runs + 1
        pixel = pixel + run
    end while
    return runs
end fn

fn xor_run_count(
    frame: array<i32>,
    previous: array<i32>,
    shape: tile_shape,
    frame_width: i32
) -> i32
    let mut runs: i32 = 0
    let mut offset: i32 = 0
    while offset < shape.byte_count
        let source_index = frame_index_for_tile_byte(shape, frame_width, offset)
        let value = frame[source_index] ^ previous[source_index]
        let mut run: i32 = 1
        while offset + run < shape.byte_count && run < 65535
            let next_index = frame_index_for_tile_byte(shape, frame_width, offset + run)
            let next_value = frame[next_index] ^ previous[next_index]
            if next_value != value then
                break
            end if
            run = run + 1
        end while
        runs = runs + 1
        offset = offset + run
    end while
    return runs
end fn

fn choose_tile(
    frame: array<i32>,
    previous: array<i32>,
    has_previous: bool,
    shape: tile_shape,
    frame_width: i32
) -> tile_choice
    let mut best_mode = tile_mode.raw
    let mut best_size = shape.byte_count

    # Evaluate in reverse tie priority so <= selects the canonical winner.
    if has_previous then
        let xor_size = xor_run_count(frame, previous, shape, frame_width) * 3
        if xor_size <= best_size then
            best_mode = tile_mode.xor_rle
            best_size = xor_size
        end if
    end if

    let rle_size = pixel_run_count(frame, shape, frame_width) * 5
    if rle_size <= best_size then
        best_mode = tile_mode.pixel_rle
        best_size = rle_size
    end if

    if tile_is_solid(frame, shape, frame_width) && 3 <= best_size then
        best_mode = tile_mode.solid
        best_size = 3
    end if

    if has_previous && tile_matches_previous(frame, previous, shape, frame_width) then
        best_mode = tile_mode.unchanged
        best_size = 0
    end if

    return tile_choice { mode: best_mode, payload_size: best_size }
end fn

fn write_u16_le(output: ptr<i32>, offset: i32, value: i32) -> i32
    output[offset] = value & 255
    output[offset + 1] = (value >> 8) & 255
    return offset + 2
end fn

fn write_u32_le(output: ptr<i32>, offset: i32, value: i32) -> i32
    output[offset] = value & 255
    output[offset + 1] = (value >> 8) & 255
    output[offset + 2] = (value >> 16) & 255
    output[offset + 3] = (value >> 24) & 255
    return offset + 4
end fn

fn read_u16_le(input: ptr<i32>, offset: i32) -> i32
    return input[offset] | (input[offset + 1] << 8)
end fn

fn read_u32_le(input: ptr<i32>, offset: i32) -> i64
    return input[offset] as i64 |
        ((input[offset + 1] as i64) << 8) |
        ((input[offset + 2] as i64) << 16) |
        ((input[offset + 3] as i64) << 24)
end fn

fn failed_encode(status: codec_status) -> encoded_payload
    return encoded_payload {
        data: allocate<i32>(1),
        length: 0,
        modes: allocate<i32>(1),
        mode_count: 0,
        status: status
    }
end fn

pub fn encode_frame_payload(
    frame: array<i32>,
    previous: array<i32>,
    has_previous: bool,
    width: i32,
    height: i32,
    tile_width: i32,
    tile_height: i32
) -> encoded_payload
    # The previous frame is read only when has_previous is true.
    if not geometry_is_valid(width, height, tile_width, tile_height) then
        return failed_encode(codec_status.invalid_geometry)
    end if
    let expected_size = frame_size(width, height)
    if expected_size > limits::maximum_frame_bytes() then
        return failed_encode(codec_status.resource_limit)
    end if
    let columns = tile_columns(width, tile_width)
    let rows = tile_rows(height, tile_height)
    let tile_count = columns * rows
    let capacity = expected_size + tile_count * 5
    if capacity > limits::maximum_payload_bytes() then
        return failed_encode(codec_status.resource_limit)
    end if
    if frame.len() != expected_size then
        return failed_encode(codec_status.invalid_frame_length)
    end if
    if not array_contains_only_octets(frame) then
        return failed_encode(codec_status.invalid_octet)
    end if
    if has_previous && previous.len() != expected_size then
        return failed_encode(codec_status.missing_previous)
    end if
    if has_previous && not array_contains_only_octets(previous) then
        return failed_encode(codec_status.invalid_octet)
    end if

    let output = allocate<i32>(capacity)
    let modes = allocate<i32>(tile_count)
    let mut output_offset: i32 = 0
    let mut tile_number: i32 = 0

    while tile_number < tile_count
        let shape = shape_for_tile(width, height, tile_width, tile_height, tile_number)
        let choice = choose_tile(frame, previous, has_previous, shape, width)
        let code = mode_code(choice.mode)
        modes[tile_number] = code
        output[output_offset] = code
        output_offset = write_u32_le(output, output_offset + 1, choice.payload_size)

        if code == 0 then
            let mut byte_offset: i32 = 0
            while byte_offset < shape.byte_count
                output[output_offset] = tile_value(frame, shape, width, byte_offset)
                output_offset = output_offset + 1
                byte_offset = byte_offset + 1
            end while
        elseif code == 1 then
            output[output_offset] = tile_value(frame, shape, width, 0)
            output[output_offset + 1] = tile_value(frame, shape, width, 1)
            output[output_offset + 2] = tile_value(frame, shape, width, 2)
            output_offset = output_offset + 3
        elseif code == 2 then
            let pixel_count = shape.byte_count / 3
            let mut pixel: i32 = 0
            while pixel < pixel_count
                let mut run: i32 = 1
                while pixel + run < pixel_count && run < 65535 && same_pixel(frame, shape, width, pixel, pixel + run)
                    run = run + 1
                end while
                output_offset = write_u16_le(output, output_offset, run)
                output[output_offset] = tile_value(frame, shape, width, pixel * 3)
                output[output_offset + 1] = tile_value(frame, shape, width, pixel * 3 + 1)
                output[output_offset + 2] = tile_value(frame, shape, width, pixel * 3 + 2)
                output_offset = output_offset + 3
                pixel = pixel + run
            end while
        elseif code == 4 then
            let mut byte_offset: i32 = 0
            while byte_offset < shape.byte_count
                let source_index = frame_index_for_tile_byte(shape, width, byte_offset)
                let value = frame[source_index] ^ previous[source_index]
                let mut run: i32 = 1
                while byte_offset + run < shape.byte_count && run < 65535
                    let next_index = frame_index_for_tile_byte(shape, width, byte_offset + run)
                    let next_value = frame[next_index] ^ previous[next_index]
                    if next_value != value then
                        break
                    end if
                    run = run + 1
                end while
                output_offset = write_u16_le(output, output_offset, run)
                output[output_offset] = value
                output_offset = output_offset + 1
                byte_offset = byte_offset + run
            end while
        end if

        tile_number = tile_number + 1
    end while

    return encoded_payload {
        data: output,
        length: output_offset,
        modes: modes,
        mode_count: tile_count,
        status: codec_status.ok
    }
end fn

fn failed_decode(status: codec_status) -> decoded_frame
    return decoded_frame {
        pixels: allocate<i32>(1),
        length: 0,
        modes: allocate<i32>(1),
        mode_count: 0,
        status: status
    }
end fn

fn fail_decode_after_allocation(
    pixels: ptr<i32>,
    modes: ptr<i32>,
    status: codec_status
) -> decoded_frame
    deallocate(pixels)
    deallocate(modes)
    return failed_decode(status)
end fn

fn pointer_contains_only_octets(input: ptr<i32>, length: i32) -> bool
    let mut index: i32 = 0
    while index < length
        if not octet_is_valid(input[index]) then
            return false
        end if
        index = index + 1
    end while
    return true
end fn

pub fn decode_frame_payload(
    payload: ptr<i32>,
    payload_length: i32,
    previous: array<i32>,
    key_frame: bool,
    width: i32,
    height: i32,
    tile_width: i32,
    tile_height: i32
) -> decoded_frame
    # payload_length is the caller-proven extent of payload's allocation.
    if not geometry_is_valid(width, height, tile_width, tile_height) then
        return failed_decode(codec_status.invalid_geometry)
    end if
    let expected_size = frame_size(width, height)
    if expected_size > limits::maximum_frame_bytes() then
        return failed_decode(codec_status.resource_limit)
    end if
    if payload_length < 0 then
        return failed_decode(codec_status.truncated)
    end if
    if payload_length > limits::maximum_payload_bytes() then
        return failed_decode(codec_status.resource_limit)
    end if
    let columns = tile_columns(width, tile_width)
    let tile_count = columns * tile_rows(height, tile_height)
    if payload_length < tile_count * 5 then
        return failed_decode(codec_status.truncated)
    end if
    if not pointer_contains_only_octets(payload, payload_length) then
        return failed_decode(codec_status.invalid_octet)
    end if

    if not key_frame && previous.len() != expected_size then
        return failed_decode(codec_status.missing_previous)
    end if
    if not key_frame && not array_contains_only_octets(previous) then
        return failed_decode(codec_status.invalid_octet)
    end if

    let pixels = allocate<i32>(expected_size)
    let modes = allocate<i32>(tile_count)
    let mut payload_offset: i32 = 0
    let mut tile_number: i32 = 0

    while tile_number < tile_count
        if payload_offset + 5 > payload_length then
            return fail_decode_after_allocation(pixels, modes, codec_status.truncated)
        end if
        let mode = payload[payload_offset]
        let encoded_size_64 = read_u32_le(payload, payload_offset + 1)
        payload_offset = payload_offset + 5
        if encoded_size_64 > (payload_length - payload_offset) as i64 then
            return fail_decode_after_allocation(pixels, modes, codec_status.truncated)
        end if
        let encoded_size = encoded_size_64 as i32
        let encoded_end = payload_offset + encoded_size
        let shape = shape_for_tile(width, height, tile_width, tile_height, tile_number)
        modes[tile_number] = mode

        if mode == 0 then
            if encoded_size != shape.byte_count then
                return fail_decode_after_allocation(pixels, modes, codec_status.invalid_tile_length)
            end if
            let mut tile_offset: i32 = 0
            while tile_offset < shape.byte_count
                let destination = frame_index_for_tile_byte(shape, width, tile_offset)
                pixels[destination] = payload[payload_offset + tile_offset]
                tile_offset = tile_offset + 1
            end while
        elseif mode == 1 then
            if encoded_size != 3 then
                return fail_decode_after_allocation(pixels, modes, codec_status.invalid_tile_length)
            end if
            let mut tile_offset: i32 = 0
            while tile_offset < shape.byte_count
                let destination = frame_index_for_tile_byte(shape, width, tile_offset)
                pixels[destination] = payload[payload_offset + tile_offset % 3]
                tile_offset = tile_offset + 1
            end while
        elseif mode == 2 then
            let mut encoded_offset: i32 = 0
            let mut tile_offset: i32 = 0
            while encoded_offset < encoded_size
                if encoded_offset + 5 > encoded_size then
                    return fail_decode_after_allocation(pixels, modes, codec_status.invalid_tile_length)
                end if
                let run = read_u16_le(payload, payload_offset + encoded_offset)
                if run == 0 then
                    return fail_decode_after_allocation(pixels, modes, codec_status.zero_run)
                end if
                if run > (shape.byte_count - tile_offset) / 3 then
                    return fail_decode_after_allocation(pixels, modes, codec_status.run_overflow)
                end if
                let mut run_pixel: i32 = 0
                while run_pixel < run
                    let mut component: i32 = 0
                    while component < 3
                        let destination = frame_index_for_tile_byte(shape, width, tile_offset)
                        pixels[destination] = payload[payload_offset + encoded_offset + 2 + component]
                        tile_offset = tile_offset + 1
                        component = component + 1
                    end while
                    run_pixel = run_pixel + 1
                end while
                encoded_offset = encoded_offset + 5
            end while
            if tile_offset != shape.byte_count then
                return fail_decode_after_allocation(pixels, modes, codec_status.underfill)
            end if
        elseif mode == 3 then
            if key_frame then
                return fail_decode_after_allocation(pixels, modes, codec_status.temporal_in_key)
            end if
            if encoded_size != 0 then
                return fail_decode_after_allocation(pixels, modes, codec_status.invalid_tile_length)
            end if
            let mut tile_offset: i32 = 0
            while tile_offset < shape.byte_count
                let destination = frame_index_for_tile_byte(shape, width, tile_offset)
                pixels[destination] = previous[destination]
                tile_offset = tile_offset + 1
            end while
        elseif mode == 4 then
            if key_frame then
                return fail_decode_after_allocation(pixels, modes, codec_status.temporal_in_key)
            end if
            let mut encoded_offset: i32 = 0
            let mut tile_offset: i32 = 0
            while encoded_offset < encoded_size
                if encoded_offset + 3 > encoded_size then
                    return fail_decode_after_allocation(pixels, modes, codec_status.invalid_tile_length)
                end if
                let run = read_u16_le(payload, payload_offset + encoded_offset)
                if run == 0 then
                    return fail_decode_after_allocation(pixels, modes, codec_status.zero_run)
                end if
                if run > shape.byte_count - tile_offset then
                    return fail_decode_after_allocation(pixels, modes, codec_status.run_overflow)
                end if
                let delta = payload[payload_offset + encoded_offset + 2]
                let mut run_byte: i32 = 0
                while run_byte < run
                    let destination = frame_index_for_tile_byte(shape, width, tile_offset)
                    pixels[destination] = previous[destination] ^ delta
                    tile_offset = tile_offset + 1
                    run_byte = run_byte + 1
                end while
                encoded_offset = encoded_offset + 3
            end while
            if tile_offset != shape.byte_count then
                return fail_decode_after_allocation(pixels, modes, codec_status.underfill)
            end if
        else
            return fail_decode_after_allocation(pixels, modes, codec_status.unknown_mode)
        end if

        payload_offset = encoded_end
        tile_number = tile_number + 1
    end while

    if payload_offset != payload_length then
        return fail_decode_after_allocation(pixels, modes, codec_status.trailing_data)
    end if

    return decoded_frame {
        pixels: pixels,
        length: expected_size,
        modes: modes,
        mode_count: tile_count,
        status: codec_status.ok
    }
end fn

pub fn crc32c(values: array<i32>) -> i64
    if not array_contains_only_octets(values) then
        return -1
    end if
    let mut accumulator: i64 = 0xffffffff
    let mut index: i32 = 0
    while index < values.len()
        accumulator = accumulator ^ (values[index] as i64)
        let mut bit: i32 = 0
        while bit < 8
            if (accumulator & 1) == 1 then
                accumulator = (accumulator >> 1) ^ 0x82f63b78
            else
                accumulator = accumulator >> 1
            end if
            bit = bit + 1
        end while
        index = index + 1
    end while
    return (accumulator ^ 0xffffffff) & 0xffffffff
end fn

fn crc32c_pointer(values: ptr<i32>, length: i32) -> i64
    if not pointer_contains_only_octets(values, length) then
        return -1
    end if
    let mut accumulator: i64 = 0xffffffff
    let mut index: i32 = 0
    while index < length
        accumulator = accumulator ^ (values[index] as i64)
        let mut bit: i32 = 0
        while bit < 8
            if (accumulator & 1) == 1 then
                accumulator = (accumulator >> 1) ^ 0x82f63b78
            else
                accumulator = accumulator >> 1
            end if
            bit = bit + 1
        end while
        index = index + 1
    end while
    return (accumulator ^ 0xffffffff) & 0xffffffff
end fn

fn copy_array(values: array<i32>) -> ptr<i32>
    let length: i32 = values.len()
    let output = allocate<i32>(length)
    let mut index: i32 = 0
    while index < values.len()
        output[index] = values[index]
        index = index + 1
    end while
    return output
end fn

fn pointer_matches_array(values: ptr<i32>, length: i32, expected: array<i32>) -> bool
    if length != expected.len() then
        return false
    end if
    let mut index: i32 = 0
    while index < length
        if values[index] != expected[index] then
            return false
        end if
        index = index + 1
    end while
    return true
end fn

fn print_scalar(label: text, value: i64)
    println(label)
    println(value)
end fn

fn print_block(label: text, values: ptr<i32>, length: i32)
    println(label)
    println(length)
    let mut index: i32 = 0
    while index < length
        println(values[index])
        index = index + 1
    end while
end fn

pub fn release_encoded(payload: encoded_payload)
    deallocate(payload.data)
    deallocate(payload.modes)
end fn

pub fn release_decoded(frame: decoded_frame)
    deallocate(frame.pixels)
    deallocate(frame.modes)
end fn

@ai_context(
    intent="Produce exact oracle-comparable Eidolon tile payload evidence",
    invariants=[
        "Every logical octet is in 0..255",
        "Tile records fill exact clipped geometry",
        "Temporal modes read only the caller-provided previous frame"
    ],
    privacy="Output contains only committed synthetic conformance pixels"
)
pub fn run_conformance() -> i32
    let crc_input: array<i32> = [49, 50, 51, 52, 53, 54, 55, 56, 57]
    let key_frame: array<i32> = [
        0, 0, 0, 41, 3, 29, 82, 6, 58, 123, 9, 87, 164, 12, 116,
        7, 83, 29, 48, 86, 58, 89, 89, 87, 130, 92, 116, 171, 95, 145,
        14, 166, 58, 55, 169, 87, 96, 172, 116, 137, 175, 145, 178, 178, 174
    ]
    let delta_frame: array<i32> = [
        255, 255, 255, 41, 3, 29, 82, 6, 58, 123, 9, 87, 164, 12, 116,
        7, 83, 29, 48, 86, 58, 7, 7, 7, 130, 92, 116, 171, 95, 145,
        14, 166, 58, 55, 169, 87, 96, 172, 116, 137, 175, 145, 0, 0, 0
    ]
    let rle_frame: array<i32> = [1, 2, 3, 1, 2, 3, 1, 2, 3, 4, 5, 6]
    let no_previous: array<i32> = [0]

    let key = encode_frame_payload(key_frame, no_previous, false, 5, 3, 4, 2)
    let delta = encode_frame_payload(delta_frame, key_frame, true, 5, 3, 4, 2)
    let rle = encode_frame_payload(rle_frame, no_previous, false, 4, 1, 4, 1)
    let decoded_key = decode_frame_payload(key.data, key.length, no_previous, true, 5, 3, 4, 2)
    let decoded_delta = decode_frame_payload(delta.data, delta.length, key_frame, false, 5, 3, 4, 2)
    let decoded_rle = decode_frame_payload(rle.data, rle.length, no_previous, true, 4, 1, 4, 1)

    print_scalar("CRC32C_STANDARD", crc32c(crc_input))
    print_scalar("KEY_STATUS", status_code(key.status) as i64)
    print_block("KEY_PAYLOAD", key.data, key.length)
    print_block("KEY_MODES", key.modes, key.mode_count)
    print_scalar("KEY_PAYLOAD_CRC32C", crc32c_pointer(key.data, key.length))
    print_scalar("DELTA_STATUS", status_code(delta.status) as i64)
    print_block("DELTA_PAYLOAD", delta.data, delta.length)
    print_block("DELTA_MODES", delta.modes, delta.mode_count)
    print_scalar("DELTA_PAYLOAD_CRC32C", crc32c_pointer(delta.data, delta.length))
    print_scalar("RLE_STATUS", status_code(rle.status) as i64)
    print_block("RLE_PAYLOAD", rle.data, rle.length)
    print_block("RLE_MODES", rle.modes, rle.mode_count)
    print_scalar("DECODED_KEY_STATUS", status_code(decoded_key.status) as i64)
    print_block("DECODED_KEY", decoded_key.pixels, decoded_key.length)
    print_scalar("DECODED_DELTA_STATUS", status_code(decoded_delta.status) as i64)
    print_block("DECODED_DELTA", decoded_delta.pixels, decoded_delta.length)
    print_scalar("DECODED_RLE_STATUS", status_code(decoded_rle.status) as i64)
    print_block("DECODED_RLE", decoded_rle.pixels, decoded_rle.length)

    let malformed_raw_values: array<i32> = [0, 11, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]
    let zero_run_values: array<i32> = [2, 5, 0, 0, 0, 0, 0, 1, 2, 3]
    let run_overflow_values: array<i32> = [2, 5, 0, 0, 0, 5, 0, 1, 2, 3]
    let unknown_mode_values: array<i32> = [255, 0, 0, 0, 0]
    let trailing_values: array<i32> = [1, 3, 0, 0, 0, 1, 2, 3, 99]
    let temporal_key_values: array<i32> = [3, 0, 0, 0, 0]
    let malformed_raw = copy_array(malformed_raw_values)
    let zero_run = copy_array(zero_run_values)
    let run_overflow = copy_array(run_overflow_values)
    let unknown_mode = copy_array(unknown_mode_values)
    let trailing = copy_array(trailing_values)
    let temporal_key = copy_array(temporal_key_values)
    let malformed_raw_result = decode_frame_payload(malformed_raw, malformed_raw_values.len(), no_previous, true, 2, 2, 2, 2)
    let zero_run_result = decode_frame_payload(zero_run, zero_run_values.len(), no_previous, true, 2, 2, 2, 2)
    let run_overflow_result = decode_frame_payload(run_overflow, run_overflow_values.len(), no_previous, true, 2, 2, 2, 2)
    let unknown_mode_result = decode_frame_payload(unknown_mode, unknown_mode_values.len(), no_previous, true, 1, 1, 1, 1)
    let trailing_result = decode_frame_payload(trailing, trailing_values.len(), no_previous, true, 1, 1, 1, 1)
    let temporal_key_result = decode_frame_payload(temporal_key, temporal_key_values.len(), no_previous, true, 1, 1, 1, 1)
    # Each probe supplies only one real element. The claimed geometry or length
    # must be rejected before scanning caller memory or allocating large output.
    let encode_frame_budget = encode_frame_payload(no_previous, no_previous, false, 1024, 768, 16, 16)
    let encode_capacity_budget = encode_frame_payload(no_previous, no_previous, false, 513, 256, 1, 1)
    let resource_pointer = copy_array(no_previous)
    let decode_frame_budget = decode_frame_payload(resource_pointer, 0, no_previous, true, 1024, 768, 16, 16)
    let decode_payload_budget = decode_frame_payload(
        resource_pointer,
        limits::maximum_payload_bytes() + 1,
        no_previous,
        true,
        1,
        1,
        1,
        1
    )
    let decode_minimum_header = decode_frame_payload(resource_pointer, 0, no_previous, true, 512, 256, 1, 1)

    print_scalar("MALFORMED_RAW_STATUS", status_code(malformed_raw_result.status) as i64)
    print_scalar("ZERO_RUN_STATUS", status_code(zero_run_result.status) as i64)
    print_scalar("RUN_OVERFLOW_STATUS", status_code(run_overflow_result.status) as i64)
    print_scalar("UNKNOWN_MODE_STATUS", status_code(unknown_mode_result.status) as i64)
    print_scalar("TRAILING_STATUS", status_code(trailing_result.status) as i64)
    print_scalar("TEMPORAL_KEY_STATUS", status_code(temporal_key_result.status) as i64)
    print_scalar("ENCODE_FRAME_BUDGET_STATUS", status_code(encode_frame_budget.status) as i64)
    print_scalar("ENCODE_CAPACITY_BUDGET_STATUS", status_code(encode_capacity_budget.status) as i64)
    print_scalar("DECODE_FRAME_BUDGET_STATUS", status_code(decode_frame_budget.status) as i64)
    print_scalar("DECODE_PAYLOAD_BUDGET_STATUS", status_code(decode_payload_budget.status) as i64)
    print_scalar("DECODE_MINIMUM_HEADER_STATUS", status_code(decode_minimum_header.status) as i64)

    let mut exit_code: i32 = 0
    if crc32c(crc_input) != 0xe3069283 then
        exit_code = 1
    end if
    if status_code(key.status) != 0 || status_code(delta.status) != 0 || status_code(rle.status) != 0 then
        exit_code = 1
    end if
    if not pointer_matches_array(decoded_key.pixels, decoded_key.length, key_frame) then
        exit_code = 1
    end if
    if not pointer_matches_array(decoded_delta.pixels, decoded_delta.length, delta_frame) then
        exit_code = 1
    end if
    if not pointer_matches_array(decoded_rle.pixels, decoded_rle.length, rle_frame) then
        exit_code = 1
    end if
    if status_code(malformed_raw_result.status) != 6 || status_code(zero_run_result.status) != 7 then
        exit_code = 1
    end if
    if status_code(run_overflow_result.status) != 8 || status_code(unknown_mode_result.status) != 5 then
        exit_code = 1
    end if
    if status_code(trailing_result.status) != 12 || status_code(temporal_key_result.status) != 10 then
        exit_code = 1
    end if
    if status_code(encode_frame_budget.status) != 13 ||
        status_code(encode_capacity_budget.status) != 13 ||
        status_code(decode_frame_budget.status) != 13 ||
        status_code(decode_payload_budget.status) != 13 ||
        status_code(decode_minimum_header.status) != 4 then
        exit_code = 1
    end if

    release_encoded(key)
    release_encoded(delta)
    release_encoded(rle)
    release_decoded(decoded_key)
    release_decoded(decoded_delta)
    release_decoded(decoded_rle)
    release_decoded(malformed_raw_result)
    release_decoded(zero_run_result)
    release_decoded(run_overflow_result)
    release_decoded(unknown_mode_result)
    release_decoded(trailing_result)
    release_decoded(temporal_key_result)
    release_encoded(encode_frame_budget)
    release_encoded(encode_capacity_budget)
    release_decoded(decode_frame_budget)
    release_decoded(decode_payload_budget)
    release_decoded(decode_minimum_header)
    deallocate(resource_pointer)
    deallocate(malformed_raw)
    deallocate(zero_run)
    deallocate(run_overflow)
    deallocate(unknown_mode)
    deallocate(trailing)
    deallocate(temporal_key)
    return exit_code
end fn
