module format

import codec
import limits

# The maximum_* compatibility accessors remain stable while `limits` owns the
# values shared by the public tile codec, container codec, PPM parser, and CLI.
# The former public max_* constants were centralized during the pre-alpha API.
pub fn maximum_width() -> i32
    return limits::maximum_width()
end fn

pub fn maximum_height() -> i32
    return limits::maximum_height()
end fn

pub fn maximum_tile_edge() -> i32
    return limits::maximum_tile_edge()
end fn

pub fn maximum_frames() -> i32
    return limits::maximum_frames()
end fn

pub fn maximum_tiles_per_frame() -> i32
    return limits::maximum_tiles_per_frame()
end fn

pub fn maximum_frame_bytes() -> i32
    return limits::maximum_frame_bytes()
end fn

pub fn maximum_payload_bytes() -> i32
    return limits::maximum_payload_bytes()
end fn

pub fn maximum_container_bytes() -> i32
    return limits::maximum_container_bytes()
end fn

pub fn maximum_total_decoded_bytes() -> i32
    return limits::maximum_total_decoded_bytes()
end fn

pub enum trace_status
    ok
    invalid_geometry
    invalid_frame_count
    invalid_frame_length
    invalid_options
    container_too_large
    invalid_file_magic
    invalid_file_version
    invalid_file_header_size
    invalid_file_crc
    invalid_file_flags
    invalid_pixel_format
    invalid_timebase
    truncated
    invalid_frame_magic
    invalid_frame_header_size
    invalid_frame_crc
    invalid_frame_flags
    invalid_frame_index
    first_frame_not_key
    zero_duration
    timestamp_regression
    invalid_uncompressed_size
    payload_too_large
    payload_crc_mismatch
    tile_payload_invalid
    decoded_crc_mismatch
    trailing_data
    invalid_octet
end enum

pub struct encoded_trace
    # Data is observable only when status is ok.
    pub data: array<i32>
    pub status: trace_status
    pub width: i32
    pub height: i32
    pub frame_count: i32
end struct

pub struct decoded_trace
    # Verified frame pixels are concatenated in frame order. A failed decode
    # returns an empty array so no candidate frame becomes observable.
    pub pixels: array<i32>
    pub status: trace_status
    pub width: i32
    pub height: i32
    pub tile_width: i32
    pub tile_height: i32
    pub timebase_numerator: i64
    pub timebase_denominator: i64
    pub frame_count: i32
    pub raw_tiles: i32
    pub solid_tiles: i32
    pub pixel_rle_tiles: i32
    pub unchanged_tiles: i32
    pub xor_rle_tiles: i32
end struct

pub fn trace_status_code(status: trace_status) -> i32
    match status
        trace_status.ok -> return 0
        trace_status.invalid_geometry -> return 1
        trace_status.invalid_frame_count -> return 2
        trace_status.invalid_frame_length -> return 3
        trace_status.invalid_options -> return 4
        trace_status.container_too_large -> return 5
        trace_status.invalid_file_magic -> return 6
        trace_status.invalid_file_version -> return 7
        trace_status.invalid_file_header_size -> return 8
        trace_status.invalid_file_crc -> return 9
        trace_status.invalid_file_flags -> return 10
        trace_status.invalid_pixel_format -> return 11
        trace_status.invalid_timebase -> return 12
        trace_status.truncated -> return 13
        trace_status.invalid_frame_magic -> return 14
        trace_status.invalid_frame_header_size -> return 15
        trace_status.invalid_frame_crc -> return 16
        trace_status.invalid_frame_flags -> return 17
        trace_status.invalid_frame_index -> return 18
        trace_status.first_frame_not_key -> return 19
        trace_status.zero_duration -> return 20
        trace_status.timestamp_regression -> return 21
        trace_status.invalid_uncompressed_size -> return 22
        trace_status.payload_too_large -> return 23
        trace_status.payload_crc_mismatch -> return 24
        trace_status.tile_payload_invalid -> return 25
        trace_status.decoded_crc_mismatch -> return 26
        trace_status.trailing_data -> return 27
        trace_status.invalid_octet -> return 28
    end match
    return 255
end fn

fn empty_i32_array() -> array<i32>
    let values: array<i32> = []
    return values
end fn

fn failed_trace_encode(status: trace_status) -> encoded_trace
    return encoded_trace {
        data: empty_i32_array(),
        status: status,
        width: 0,
        height: 0,
        frame_count: 0
    }
end fn

fn failed_trace_decode(status: trace_status) -> decoded_trace
    return decoded_trace {
        pixels: empty_i32_array(),
        status: status,
        width: 0,
        height: 0,
        tile_width: 0,
        tile_height: 0,
        timebase_numerator: 0,
        timebase_denominator: 0,
        frame_count: 0,
        raw_tiles: 0,
        solid_tiles: 0,
        pixel_rle_tiles: 0,
        unchanged_tiles: 0,
        xor_rle_tiles: 0
    }
end fn

fn append_u16_le(mut output: array<i32>, value: i32)
    output.push(value & 255)
    output.push((value >> 8) & 255)
end fn

fn append_u32_le(mut output: array<i32>, value: i64)
    output.push((value & 255) as i32)
    output.push(((value >> 8) & 255) as i32)
    output.push(((value >> 16) & 255) as i32)
    output.push(((value >> 24) & 255) as i32)
end fn

fn append_u64_le(mut output: array<i32>, value: i64)
    append_u32_le(output, value & 0xffffffff)
    append_u32_le(output, (value >> 32) & 0xffffffff)
end fn

fn read_u16_le(values: array<i32>, offset: i32) -> i32
    return values[offset] | (values[offset + 1] << 8)
end fn

fn read_u32_le(values: array<i32>, offset: i32) -> i64
    return values[offset] as i64 |
        ((values[offset + 1] as i64) << 8) |
        ((values[offset + 2] as i64) << 16) |
        ((values[offset + 3] as i64) << 24)
end fn

fn append_range(mut output: array<i32>, values: array<i32>, offset: i32, length: i32)
    let mut index: i32 = 0
    while index < length
        output.push(values[offset + index])
        index = index + 1
    end while
end fn

fn copy_range(values: array<i32>, offset: i32, length: i32) -> array<i32>
    let mut output: array<i32> = []
    append_range(output, values, offset, length)
    return output
end fn

fn pointer_to_array(values: ptr<i32>, length: i32) -> array<i32>
    let mut output: array<i32> = []
    let mut index: i32 = 0
    while index < length
        output.push(values[index])
        index = index + 1
    end while
    return output
end fn

fn array_to_pointer(values: array<i32>) -> ptr<i32>
    let storage = allocate<i32>(values.len())
    let mut index: i32 = 0
    while index < values.len()
        storage[index] = values[index]
        index = index + 1
    end while
    return storage
end fn

fn crc32c_range(values: array<i32>, offset: i32, length: i32) -> i64
    let selected = copy_range(values, offset, length)
    return codec::crc32c(selected)
end fn

fn file_magic_matches(values: array<i32>, offset: i32) -> bool
    return values[offset] == 69 && values[offset + 1] == 73 &&
        values[offset + 2] == 68 && values[offset + 3] == 79 &&
        values[offset + 4] == 76 && values[offset + 5] == 79 &&
        values[offset + 6] == 78 && values[offset + 7] == 0
end fn

fn contains_only_octets(values: array<i32>) -> bool
    let mut index: i32 = 0
    while index < values.len()
        if values[index] < 0 || values[index] > 255 then
            return false
        end if
        index = index + 1
    end while
    return true
end fn

fn frame_magic_matches(values: array<i32>, offset: i32) -> bool
    return values[offset] == 70 && values[offset + 1] == 82 &&
        values[offset + 2] == 77 && values[offset + 3] == 49
end fn

fn geometry_is_valid(width: i32, height: i32, tile_width: i32, tile_height: i32) -> bool
    if width < 1 || width > maximum_width() || height < 1 || height > maximum_height() then
        return false
    end if
    if tile_width < 1 || tile_width > maximum_tile_edge() ||
        tile_height < 1 || tile_height > maximum_tile_edge() then
        return false
    end if
    if height > maximum_frame_bytes() / width / 3 then
        return false
    end if
    let columns = (width + tile_width - 1) / tile_width
    let rows = (height + tile_height - 1) / tile_height
    if columns < 1 || rows < 1 || rows > maximum_tiles_per_frame() / columns then
        return false
    end if
    return true
end fn

fn append_file_header(
    mut output: array<i32>,
    width: i32,
    height: i32,
    tile_width: i32,
    tile_height: i32,
    timebase_numerator: i64,
    timebase_denominator: i64,
    frame_count: i32
)
    output.push(69)
    output.push(73)
    output.push(68)
    output.push(79)
    output.push(76)
    output.push(79)
    output.push(78)
    output.push(0)
    append_u16_le(output, 1)
    append_u16_le(output, 48)
    append_u32_le(output, 0)
    append_u32_le(output, width as i64)
    append_u32_le(output, height as i64)
    append_u32_le(output, 1)
    append_u16_le(output, tile_width)
    append_u16_le(output, tile_height)
    append_u32_le(output, timebase_numerator)
    append_u32_le(output, timebase_denominator)
    append_u32_le(output, frame_count as i64)
    append_u32_le(output, codec::crc32c(output))
end fn

fn append_frame_header(
    mut output: array<i32>,
    key_frame: bool,
    frame_index: i32,
    timestamp: i64,
    duration: i64,
    uncompressed_size: i32,
    payload: array<i32>,
    decoded: array<i32>
)
    let mut header: array<i32> = []
    header.push(70)
    header.push(82)
    header.push(77)
    header.push(49)
    append_u16_le(header, 44)
    if key_frame then
        append_u16_le(header, 1)
    else
        append_u16_le(header, 0)
    end if
    append_u32_le(header, frame_index as i64)
    append_u64_le(header, timestamp)
    append_u32_le(header, duration)
    append_u32_le(header, uncompressed_size as i64)
    append_u32_le(header, payload.len() as i64)
    append_u32_le(header, codec::crc32c(payload))
    append_u32_le(header, codec::crc32c(decoded))
    append_u32_le(header, codec::crc32c(header))
    append_range(output, header, 0, header.len())
end fn

@ai_context(
    intent="Encode a complete deterministic Eidolon draft-1 container",
    invariants=[
        "All frame pixels are validated before output becomes observable",
        "Frame zero and interval frames are keys",
        "Every length is checked against a local Ny resource budget",
        "Timebase and duration accept exactly the nonzero draft u32 domain"
    ],
    privacy="The codec receives only caller-selected pixels and emits no paths or pixels to diagnostics"
)
pub fn encode_trace(
    flat_pixels: array<i32>,
    frame_count: i32,
    width: i32,
    height: i32,
    tile_width: i32,
    tile_height: i32,
    timebase_numerator: i64,
    timebase_denominator: i64,
    duration: i64,
    keyframe_interval: i32
) -> encoded_trace
    if not geometry_is_valid(width, height, tile_width, tile_height) then
        return failed_trace_encode(trace_status.invalid_geometry)
    end if
    if frame_count < 1 || frame_count > maximum_frames() then
        return failed_trace_encode(trace_status.invalid_frame_count)
    end if
    if duration < 1 || duration > 4294967295 || keyframe_interval < 1 ||
        timebase_numerator < 1 || timebase_numerator > 4294967295 ||
        timebase_denominator < 1 || timebase_denominator > 4294967295 then
        return failed_trace_encode(trace_status.invalid_options)
    end if

    let frame_bytes = width * height * 3
    if frame_count > maximum_total_decoded_bytes() / frame_bytes then
        return failed_trace_encode(trace_status.invalid_frame_count)
    end if
    # geometry_is_valid bounds the frame and tile count. Use i64 for the
    # remaining header multiplication and addition before comparing budgets.
    let tile_columns = (width + tile_width - 1) / tile_width
    let tile_rows = (height + tile_height - 1) / tile_height
    let tile_count = tile_columns * tile_rows
    let maximum_payload_capacity = (frame_bytes as i64) +
        (tile_count as i64) * 5
    if maximum_payload_capacity > maximum_payload_bytes() as i64 then
        return failed_trace_encode(trace_status.payload_too_large)
    end if
    if flat_pixels.len() != frame_bytes * frame_count then
        return failed_trace_encode(trace_status.invalid_frame_length)
    end if
    if not contains_only_octets(flat_pixels) then
        return failed_trace_encode(trace_status.invalid_octet)
    end if

    let mut output: array<i32> = []
    append_file_header(
        output,
        width,
        height,
        tile_width,
        tile_height,
        timebase_numerator,
        timebase_denominator,
        frame_count
    )

    let mut frame_index: i32 = 0
    while frame_index < frame_count
        let frame = copy_range(flat_pixels, frame_index * frame_bytes, frame_bytes)
        let mut previous: array<i32> = [0]
        let key_frame = frame_index == 0 || frame_index % keyframe_interval == 0
        if not key_frame then
            previous = copy_range(flat_pixels, (frame_index - 1) * frame_bytes, frame_bytes)
        end if

        let payload = codec::encode_frame_payload(
            frame,
            previous,
            not key_frame,
            width,
            height,
            tile_width,
            tile_height
        )
        let payload_status = codec::status_code(payload.status)
        if payload_status != 0 then
            codec::release_encoded(payload)
            if payload_status == 13 then
                return failed_trace_encode(trace_status.payload_too_large)
            end if
            return failed_trace_encode(trace_status.tile_payload_invalid)
        end if
        if payload.length < 0 || payload.length > maximum_payload_bytes() then
            codec::release_encoded(payload)
            return failed_trace_encode(trace_status.payload_too_large)
        end if
        if output.len() > maximum_container_bytes() - 44 - payload.length then
            codec::release_encoded(payload)
            return failed_trace_encode(trace_status.container_too_large)
        end if

        let payload_values = pointer_to_array(payload.data, payload.length)
        let timestamp = (frame_index as i64) * duration
        append_frame_header(
            output,
            key_frame,
            frame_index,
            timestamp,
            duration,
            frame_bytes,
            payload_values,
            frame
        )
        append_range(output, payload_values, 0, payload_values.len())
        codec::release_encoded(payload)
        frame_index = frame_index + 1
    end while

    return encoded_trace {
        data: output,
        status: trace_status.ok,
        width: width,
        height: height,
        frame_count: frame_count
    }
end fn

@ai_context(
    intent="Validate and decode a complete hostile Eidolon draft-1 container",
    invariants=[
        "No candidate frame is appended before payload and decoded CRC32C pass",
        "Delta frames read only the preceding fully verified frame",
        "Unknown fields, truncation, and trailing bytes fail closed"
    ],
    privacy="Failures are status-only; container bytes, pixels, and paths are never logged"
)
pub fn decode_trace(data: array<i32>) -> decoded_trace
    if data.len() > maximum_container_bytes() then
        return failed_trace_decode(trace_status.container_too_large)
    end if
    if not contains_only_octets(data) then
        return failed_trace_decode(trace_status.invalid_octet)
    end if
    if data.len() < 48 then
        return failed_trace_decode(trace_status.truncated)
    end if
    if not file_magic_matches(data, 0) then
        return failed_trace_decode(trace_status.invalid_file_magic)
    end if
    if read_u16_le(data, 8) != 1 then
        return failed_trace_decode(trace_status.invalid_file_version)
    end if
    if read_u16_le(data, 10) != 48 then
        return failed_trace_decode(trace_status.invalid_file_header_size)
    end if
    if crc32c_range(data, 0, 44) != read_u32_le(data, 44) then
        return failed_trace_decode(trace_status.invalid_file_crc)
    end if
    if read_u32_le(data, 12) != 0 then
        return failed_trace_decode(trace_status.invalid_file_flags)
    end if
    if read_u32_le(data, 24) != 1 then
        return failed_trace_decode(trace_status.invalid_pixel_format)
    end if

    let width_64 = read_u32_le(data, 16)
    let height_64 = read_u32_le(data, 20)
    let tile_width = read_u16_le(data, 28)
    let tile_height = read_u16_le(data, 30)
    let timebase_numerator = read_u32_le(data, 32)
    let timebase_denominator = read_u32_le(data, 36)
    let frame_count_64 = read_u32_le(data, 40)
    if timebase_numerator == 0 || timebase_denominator == 0 then
        return failed_trace_decode(trace_status.invalid_timebase)
    end if
    if frame_count_64 < 1 || frame_count_64 > maximum_frames() as i64 then
        return failed_trace_decode(trace_status.invalid_frame_count)
    end if
    if width_64 > maximum_width() as i64 || height_64 > maximum_height() as i64 then
        return failed_trace_decode(trace_status.invalid_geometry)
    end if
    let width = width_64 as i32
    let height = height_64 as i32
    if not geometry_is_valid(width, height, tile_width, tile_height) then
        return failed_trace_decode(trace_status.invalid_geometry)
    end if
    let frame_count = frame_count_64 as i32
    let frame_bytes = width * height * 3
    if frame_count > maximum_total_decoded_bytes() / frame_bytes then
        return failed_trace_decode(trace_status.invalid_frame_count)
    end if
    let tile_columns = (width + tile_width - 1) / tile_width
    let tile_rows = (height + tile_height - 1) / tile_height
    let tile_count = tile_columns * tile_rows

    let mut trace = decoded_trace {
        pixels: empty_i32_array(),
        status: trace_status.ok,
        width: width,
        height: height,
        tile_width: tile_width,
        tile_height: tile_height,
        timebase_numerator: timebase_numerator,
        timebase_denominator: timebase_denominator,
        frame_count: frame_count,
        raw_tiles: 0,
        solid_tiles: 0,
        pixel_rle_tiles: 0,
        unchanged_tiles: 0,
        xor_rle_tiles: 0
    }
    let mut offset: i32 = 48
    let mut frame_index: i32 = 0
    let mut previous_timestamp_low: i64 = 0
    let mut previous_timestamp_high: i64 = 0

    while frame_index < frame_count
        if offset > data.len() - 44 then
            return failed_trace_decode(trace_status.truncated)
        end if
        if not frame_magic_matches(data, offset) then
            return failed_trace_decode(trace_status.invalid_frame_magic)
        end if
        if read_u16_le(data, offset + 4) != 44 then
            return failed_trace_decode(trace_status.invalid_frame_header_size)
        end if
        if crc32c_range(data, offset, 40) != read_u32_le(data, offset + 40) then
            return failed_trace_decode(trace_status.invalid_frame_crc)
        end if

        let flags = read_u16_le(data, offset + 6)
        if flags > 1 then
            return failed_trace_decode(trace_status.invalid_frame_flags)
        end if
        if read_u32_le(data, offset + 8) != frame_index as i64 then
            return failed_trace_decode(trace_status.invalid_frame_index)
        end if
        let key_frame = (flags & 1) == 1
        if frame_index == 0 && not key_frame then
            return failed_trace_decode(trace_status.first_frame_not_key)
        end if

        let timestamp_low = read_u32_le(data, offset + 12)
        let timestamp_high = read_u32_le(data, offset + 16)
        if read_u32_le(data, offset + 20) == 0 then
            return failed_trace_decode(trace_status.zero_duration)
        end if
        if frame_index > 0 && (timestamp_high < previous_timestamp_high ||
            (timestamp_high == previous_timestamp_high && timestamp_low < previous_timestamp_low)) then
            return failed_trace_decode(trace_status.timestamp_regression)
        end if
        previous_timestamp_low = timestamp_low
        previous_timestamp_high = timestamp_high

        if read_u32_le(data, offset + 24) != frame_bytes as i64 then
            return failed_trace_decode(trace_status.invalid_uncompressed_size)
        end if
        let payload_size_64 = read_u32_le(data, offset + 28)
        if payload_size_64 > maximum_payload_bytes() as i64 then
            return failed_trace_decode(trace_status.payload_too_large)
        end if
        let payload_size = payload_size_64 as i32
        let payload_offset = offset + 44
        if payload_offset > data.len() || payload_size > data.len() - payload_offset then
            return failed_trace_decode(trace_status.truncated)
        end if
        if crc32c_range(data, payload_offset, payload_size) != read_u32_le(data, offset + 32) then
            return failed_trace_decode(trace_status.payload_crc_mismatch)
        end if

        let payload_values = copy_range(data, payload_offset, payload_size)
        let payload_pointer = array_to_pointer(payload_values)
        let mut previous: array<i32> = [0]
        if not key_frame then
            if frame_index < 1 then
                deallocate(payload_pointer)
                return failed_trace_decode(trace_status.first_frame_not_key)
            end if
            previous = copy_range(trace.pixels, (frame_index - 1) * frame_bytes, frame_bytes)
        end if
        let candidate = codec::decode_frame_payload(
            payload_pointer,
            payload_size,
            previous,
            key_frame,
            width,
            height,
            tile_width,
            tile_height
        )
        deallocate(payload_pointer)
        if codec::status_code(candidate.status) != 0 then
            codec::release_decoded(candidate)
            return failed_trace_decode(trace_status.tile_payload_invalid)
        end if
        if candidate.length != frame_bytes || candidate.mode_count != tile_count then
            codec::release_decoded(candidate)
            return failed_trace_decode(trace_status.tile_payload_invalid)
        end if
        let candidate_pixels = pointer_to_array(candidate.pixels, candidate.length)
        if codec::crc32c(candidate_pixels) != read_u32_le(data, offset + 36) then
            codec::release_decoded(candidate)
            return failed_trace_decode(trace_status.decoded_crc_mismatch)
        end if

        let mut mode_index: i32 = 0
        while mode_index < candidate.mode_count
            let mode = candidate.modes[mode_index]
            if mode == 0 then
                trace.raw_tiles = trace.raw_tiles + 1
            elseif mode == 1 then
                trace.solid_tiles = trace.solid_tiles + 1
            elseif mode == 2 then
                trace.pixel_rle_tiles = trace.pixel_rle_tiles + 1
            elseif mode == 3 then
                trace.unchanged_tiles = trace.unchanged_tiles + 1
            elseif mode == 4 then
                trace.xor_rle_tiles = trace.xor_rle_tiles + 1
            else
                codec::release_decoded(candidate)
                return failed_trace_decode(trace_status.tile_payload_invalid)
            end if
            mode_index = mode_index + 1
        end while
        append_range(trace.pixels, candidate_pixels, 0, candidate_pixels.len())
        codec::release_decoded(candidate)
        offset = payload_offset + payload_size
        frame_index = frame_index + 1
    end while

    if offset != data.len() then
        return failed_trace_decode(trace_status.trailing_data)
    end if
    return trace
end fn

pub fn frame_pixels(trace: decoded_trace, frame_index: i32) -> array<i32>
    if trace_status_code(trace.status) != 0 || frame_index < 0 || frame_index >= trace.frame_count then
        return empty_i32_array()
    end if
    let frame_bytes = trace.width * trace.height * 3
    return copy_range(trace.pixels, frame_index * frame_bytes, frame_bytes)
end fn
