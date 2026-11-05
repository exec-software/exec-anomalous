module ppm

import format

pub const header_limit: i32 = 4096

pub fn maximum_header_bytes() -> i32
    return header_limit
end fn

pub enum ppm_status
    ok
    invalid_octet
    header_too_large
    truncated_header
    unsupported_magic
    non_decimal
    invalid_maximum
    invalid_geometry
    invalid_raster_length
end enum

pub struct ppm_image
    # Pixels are visible only when status is ok.
    pub pixels: array<i32>
    pub status: ppm_status
    pub width: i32
    pub height: i32
end struct

pub struct ppm_document
    pub data: array<i32>
    pub status: ppm_status
end struct

struct token_result
    start: i32
    finish: i32
    status: ppm_status
end struct

struct number_result
    value: i32
    status: ppm_status
    overflow: bool
end struct

pub fn ppm_status_code(status: ppm_status) -> i32
    match status
        ppm_status.ok -> return 0
        ppm_status.invalid_octet -> return 1
        ppm_status.header_too_large -> return 2
        ppm_status.truncated_header -> return 3
        ppm_status.unsupported_magic -> return 4
        ppm_status.non_decimal -> return 5
        ppm_status.invalid_maximum -> return 6
        ppm_status.invalid_geometry -> return 7
        ppm_status.invalid_raster_length -> return 8
    end match
    return 255
end fn

# This checks only whether a caller-provided input extent can contain a legal
# profile-bounded PPM. Oversized input is a raster-length failure: the parser
# has not inspected enough bytes to claim that the header itself is too large.
pub fn input_extent_status(length: i32) -> ppm_status
    let maximum_input_extent = (format::maximum_frame_bytes() as i64) +
        (header_limit as i64)
    if length < 0 || length as i64 > maximum_input_extent then
        return ppm_status.invalid_raster_length
    end if
    return ppm_status.ok
end fn

fn empty_i32_array() -> array<i32>
    let values: array<i32> = []
    return values
end fn

fn failed_image(status: ppm_status) -> ppm_image
    return ppm_image {
        pixels: empty_i32_array(),
        status: status,
        width: 0,
        height: 0
    }
end fn

fn failed_document(status: ppm_status) -> ppm_document
    return ppm_document { data: empty_i32_array(), status: status }
end fn

fn is_whitespace(value: i32) -> bool
    return value == 32 || value == 9 || value == 13 || value == 10 || value == 11 || value == 12
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

fn find_token(values: array<i32>, initial_offset: i32) -> token_result
    let mut offset = initial_offset
    let mut skipping = true
    while skipping
        if offset >= values.len() then
            return token_result { start: 0, finish: 0, status: ppm_status.truncated_header }
        end if
        if offset >= header_limit then
            return token_result { start: 0, finish: 0, status: ppm_status.header_too_large }
        end if
        let value = values[offset]
        if is_whitespace(value) then
            offset = offset + 1
        elseif value == 35 then
            offset = offset + 1
            while offset < values.len() && values[offset] != 10
                if offset >= header_limit then
                    return token_result { start: 0, finish: 0, status: ppm_status.header_too_large }
                end if
                offset = offset + 1
            end while
            if offset >= values.len() then
                if offset >= header_limit then
                    return token_result { start: 0, finish: 0, status: ppm_status.header_too_large }
                end if
                return token_result { start: 0, finish: 0, status: ppm_status.truncated_header }
            end if
            offset = offset + 1
        else
            skipping = false
        end if
    end while

    let start = offset
    while offset < values.len() && not is_whitespace(values[offset]) && values[offset] != 35
        if offset >= header_limit then
            return token_result { start: 0, finish: 0, status: ppm_status.header_too_large }
        end if
        offset = offset + 1
    end while
    if offset == start then
        return token_result { start: 0, finish: 0, status: ppm_status.truncated_header }
    end if
    return token_result { start: start, finish: offset, status: ppm_status.ok }
end fn

fn parse_decimal(values: array<i32>, item: token_result) -> number_result
    if ppm_status_code(item.status) != 0 || item.finish <= item.start then
        return number_result { value: 0, status: ppm_status.non_decimal, overflow: false }
    end if
    let mut value: i32 = 0
    let mut overflow = false
    let mut offset = item.start
    while offset < item.finish
        let encoded_digit = values[offset]
        let digit: i32 = encoded_digit - 48
        if digit < 0 || digit > 9 then
            return number_result { value: 0, status: ppm_status.non_decimal, overflow: false }
        end if
        if not overflow then
            if value > (2147483647 - digit) / 10 then
                overflow = true
            else
                value = value * 10 + digit
            end if
        end if
        offset = offset + 1
    end while
    return number_result { value: value, status: ppm_status.ok, overflow: overflow }
end fn

@ai_context(
    intent="Parse the strict P6 RGB8 subset emitted by QEMU screendump",
    invariants=[
        "Header scanning never passes 4096 bytes",
        "Geometry is bounded before multiplication",
        "The raster has exactly width times height times three bytes"
    ],
    privacy="No token, path, comment, or pixel is emitted on failure"
)
pub fn parse_ppm(values: array<i32>) -> ppm_image
    let extent_status = input_extent_status(values.len())
    if ppm_status_code(extent_status) != 0 then
        return failed_image(extent_status)
    end if
    if not contains_only_octets(values) then
        return failed_image(ppm_status.invalid_octet)
    end if
    if values.len() < 2 then
        return failed_image(ppm_status.truncated_header)
    end if
    if values[0] != 80 || values[1] != 54 then
        return failed_image(ppm_status.unsupported_magic)
    end if

    let magic = find_token(values, 0)
    if ppm_status_code(magic.status) != 0 then
        return failed_image(magic.status)
    end if
    if magic.finish - magic.start != 2 || values[magic.start] != 80 || values[magic.start + 1] != 54 then
        return failed_image(ppm_status.unsupported_magic)
    end if

    let width_token = find_token(values, magic.finish)
    if ppm_status_code(width_token.status) != 0 then
        return failed_image(width_token.status)
    end if
    let height_token = find_token(values, width_token.finish)
    if ppm_status_code(height_token.status) != 0 then
        return failed_image(height_token.status)
    end if
    let maximum_token = find_token(values, height_token.finish)
    if ppm_status_code(maximum_token.status) != 0 then
        return failed_image(maximum_token.status)
    end if

    let width_result = parse_decimal(values, width_token)
    let height_result = parse_decimal(values, height_token)
    let maximum_result = parse_decimal(values, maximum_token)
    if ppm_status_code(width_result.status) != 0 || ppm_status_code(height_result.status) != 0 ||
        ppm_status_code(maximum_result.status) != 0 then
        return failed_image(ppm_status.non_decimal)
    end if
    if maximum_result.overflow then
        return failed_image(ppm_status.invalid_maximum)
    end if
    if maximum_result.value != 255 then
        return failed_image(ppm_status.invalid_maximum)
    end if
    if width_result.overflow || height_result.overflow then
        return failed_image(ppm_status.invalid_geometry)
    end if
    let width = width_result.value
    let height = height_result.value
    if width < 1 || width > format::maximum_width() || height < 1 || height > format::maximum_height() then
        return failed_image(ppm_status.invalid_geometry)
    end if
    if height > format::maximum_frame_bytes() / width / 3 then
        return failed_image(ppm_status.invalid_geometry)
    end if

    let frame_bytes = width * height * 3
    let mut raster_offset = maximum_token.finish
    # The mandatory separator is part of the bounded header. One whitespace
    # byte may end at offset 4096; a CRLF must fit within that extent.
    if raster_offset >= header_limit then
        return failed_image(ppm_status.header_too_large)
    end if
    if raster_offset >= values.len() || not is_whitespace(values[raster_offset]) then
        return failed_image(ppm_status.truncated_header)
    end if
    # CR is also a complete one-byte separator. A following LF belongs to the
    # raster when consuming only CR leaves the exact declared byte count.
    if values[raster_offset] == 13 && raster_offset + 1 < values.len() &&
        values[raster_offset + 1] == 10 &&
        values.len() - (raster_offset + 1) != frame_bytes then
        raster_offset = raster_offset + 2
    else
        raster_offset = raster_offset + 1
    end if
    if raster_offset > header_limit then
        return failed_image(ppm_status.header_too_large)
    end if

    if values.len() - raster_offset != frame_bytes then
        return failed_image(ppm_status.invalid_raster_length)
    end if
    let mut pixels: array<i32> = []
    let mut offset = raster_offset
    while offset < values.len()
        pixels.push(values[offset])
        offset = offset + 1
    end while
    return ppm_image {
        pixels: pixels,
        status: ppm_status.ok,
        width: width,
        height: height
    }
end fn

fn append_decimal(mut output: array<i32>, value: i32)
    let mut divisor: i32 = 1
    while value / divisor >= 10
        divisor = divisor * 10
    end while
    while divisor > 0
        output.push(48 + (value / divisor) % 10)
        divisor = divisor / 10
    end while
end fn

@ai_context(
    intent="Construct one canonical P6 RGB8 document from a verified frame",
    invariants=["The header is canonical ASCII", "The raster length and every octet are exact"],
    privacy="The returned document is written only through the caller's atomic publication boundary"
)
pub fn encode_ppm(width: i32, height: i32, pixels: array<i32>) -> ppm_document
    if width < 1 || width > format::maximum_width() || height < 1 || height > format::maximum_height() then
        return failed_document(ppm_status.invalid_geometry)
    end if
    if height > format::maximum_frame_bytes() / width / 3 then
        return failed_document(ppm_status.invalid_geometry)
    end if
    if pixels.len() != width * height * 3 then
        return failed_document(ppm_status.invalid_raster_length)
    end if
    if not contains_only_octets(pixels) then
        return failed_document(ppm_status.invalid_octet)
    end if

    let mut output: array<i32> = [80, 54, 10]
    append_decimal(output, width)
    output.push(32)
    append_decimal(output, height)
    output.push(10)
    output.push(50)
    output.push(53)
    output.push(53)
    output.push(10)
    let mut index: i32 = 0
    while index < pixels.len()
        output.push(pixels[index])
        index = index + 1
    end while
    return ppm_document { data: output, status: ppm_status.ok }
end fn
