module eidolon.cli

import codec
import format
import ppm
import ny.lang.core.*
import ny.lang.io.console
import ny.lang.io.file
import ny.lang.system.std_sys

struct loaded_file
    data: array<i32>
    ok: bool
end struct

struct parsed_index
    value: i32
    ok: bool
end struct

fn empty_i32_array() -> array<i32>
    let values: array<i32> = []
    return values
end fn

fn to_logical_octets(values: array<u8>) -> array<i32>
    let mut output: array<i32> = []
    let mut index: i32 = 0
    while index < values.len()
        output.push(values[index] as i32)
        index = index + 1
    end while
    return output
end fn

fn to_wire_octets(values: array<i32>) -> array<u8>
    let mut output: array<u8> = []
    let mut index: i32 = 0
    while index < values.len()
        if values[index] < 0 || values[index] > 255 then
            let empty: array<u8> = []
            return empty
        end if
        output.push(values[index] as u8)
        index = index + 1
    end while
    return output
end fn

fn load_regular(path: text, maximum_bytes: i64) -> loaded_file
    let result = ny::lang::io::read_regular_bytes(path, maximum_bytes)
    match result
        Result.Ok(bytes) ->
            return loaded_file { data: to_logical_octets(bytes), ok: true }
        Result.Error(_) ->
            return loaded_file { data: empty_i32_array(), ok: false }
    end match
end fn

fn write_atomic(path: text, values: array<i32>, replace: bool) -> bool
    let bytes = to_wire_octets(values)
    if values.len() != bytes.len() then
        return false
    end if
    let result = ny::lang::io::write_bytes_atomic(path, bytes, replace)
    match result
        Result.Ok(written) -> return written
        Result.Error(_) -> return false
    end match
end fn

fn parse_index(value: text) -> parsed_index
    let result = Convert.try_to_i32(value)
    match result
        Result.Ok(index) ->
            if index < 0 then
                return parsed_index { value: 0, ok: false }
            end if
            return parsed_index { value: index, ok: true }
        Result.Error(_) -> return parsed_index { value: 0, ok: false }
    end match
end fn

fn print_i32_field(name: text, value: i32)
    println(name + "=" + Convert.i32_to_text(value))
end fn

fn print_i64_field(name: text, value: i64)
    println(name + "=" + Convert.i64_to_text(value))
end fn

fn fail(operation: text, stage: text, status: i32, exit_code: i32) -> i32
    ny::lang::io::write_error(
        "eidolon: " + operation + " failed at " + stage +
        " (status " + Convert.i32_to_text(status) + ")\n"
    )
    return exit_code
end fn

fn usage() -> i32
    ny::lang::io::write_error(
        "usage: eidolon encode OUTPUT.eidolon FRAME.ppm [...]\n" +
        "       eidolon inspect INPUT.eidolon\n" +
        "       eidolon decode-frame INPUT.eidolon INDEX OUTPUT.ppm\n"
    )
    return 64
end fn

@ai_context(
    intent="Read explicit P6 frames and atomically publish one complete draft-1 trace",
    invariants=[
        "Every input is a bounded regular file opened without following symlinks",
        "All frames share exact geometry",
        "The destination changes only by atomic replacement after complete encoding"
    ],
    privacy="Failure output contains a stage and status only, never a caller path or pixel"
)
fn encode_command() -> i32
    let count = ny::lang::system::arg_count()
    if count < 3 then
        return usage()
    end if
    let frame_count_64 = count - 2
    if frame_count_64 < 1 || frame_count_64 > format::maximum_frames() as i64 then
        return fail("encode", "arguments", 1, 65)
    end if
    let frame_count = frame_count_64 as i32

    let mut width: i32 = 0
    let mut height: i32 = 0
    let mut flat_pixels: array<i32> = []
    let mut frame_index: i32 = 0
    while frame_index < frame_count
        let path = ny::lang::system::arg((frame_index + 2) as i64)
        let loaded = load_regular(path, (format::maximum_frame_bytes() as i64) + (ppm::maximum_header_bytes() as i64))
        if not loaded.ok then
            return fail("encode", "read", 1, 66)
        end if
        let image = ppm::parse_ppm(loaded.data)
        if ppm::ppm_status_code(image.status) != 0 then
            return fail("encode", "ppm", ppm::ppm_status_code(image.status), 66)
        end if
        if frame_index == 0 then
            width = image.width
            height = image.height
            let frame_bytes = width * height * 3
            if frame_count > format::maximum_total_decoded_bytes() / frame_bytes then
                return fail("encode", "budget", 1, 66)
            end if
        elseif image.width != width || image.height != height then
            return fail("encode", "geometry", 1, 66)
        end if

        let mut byte_index: i32 = 0
        while byte_index < image.pixels.len()
            flat_pixels.push(image.pixels[byte_index])
            byte_index = byte_index + 1
        end while
        frame_index = frame_index + 1
    end while

    let encoded = format::encode_trace(
        flat_pixels,
        frame_count,
        width,
        height,
        16,
        16,
        1,
        1000,
        16,
        120
    )
    if format::trace_status_code(encoded.status) != 0 then
        return fail("encode", "container", format::trace_status_code(encoded.status), 67)
    end if
    if not write_atomic(ny::lang::system::arg(1), encoded.data, true) then
        return fail("encode", "publish", 1, 68)
    end if

    println("operation=encode")
    println("status=ok")
    print_i32_field("width", width)
    print_i32_field("height", height)
    print_i32_field("frames", frame_count)
    print_i32_field("bytes", encoded.data.len())
    print_i64_field("crc32c", codec::crc32c(encoded.data))
    return 0
end fn

fn inspect_command() -> i32
    if ny::lang::system::arg_count() != 2 then
        return usage()
    end if
    let loaded = load_regular(
        ny::lang::system::arg(1),
        format::maximum_container_bytes() as i64
    )
    if not loaded.ok then
        return fail("inspect", "read", 1, 66)
    end if
    let trace = format::decode_trace(loaded.data)
    if format::trace_status_code(trace.status) != 0 then
        return fail("inspect", "container", format::trace_status_code(trace.status), 67)
    end if

    println("operation=inspect")
    println("status=ok")
    print_i32_field("bytes", loaded.data.len())
    print_i32_field("width", trace.width)
    print_i32_field("height", trace.height)
    print_i32_field("frames", trace.frame_count)
    print_i32_field("tile_width", trace.tile_width)
    print_i32_field("tile_height", trace.tile_height)
    print_i64_field("timebase_numerator", trace.timebase_numerator)
    print_i64_field("timebase_denominator", trace.timebase_denominator)
    print_i32_field("raw", trace.raw_tiles)
    print_i32_field("solid", trace.solid_tiles)
    print_i32_field("pixel_rle", trace.pixel_rle_tiles)
    print_i32_field("unchanged", trace.unchanged_tiles)
    print_i32_field("xor_rle", trace.xor_rle_tiles)
    print_i64_field("crc32c", codec::crc32c(loaded.data))
    return 0
end fn

@ai_context(
    intent="Validate a complete hostile trace and publish one requested P6 frame",
    invariants=[
        "The complete container and every preceding frame are verified before selection",
        "The selected output is canonical P6",
        "Publication is atomic and refuses every pre-existing destination entry"
    ],
    privacy="Failure output never includes input path, output path, header bytes, or pixels"
)
fn decode_frame_command() -> i32
    if ny::lang::system::arg_count() != 4 then
        return usage()
    end if
    let selected = parse_index(ny::lang::system::arg(2))
    if not selected.ok then
        return fail("decode-frame", "index", 1, 65)
    end if
    let loaded = load_regular(
        ny::lang::system::arg(1),
        format::maximum_container_bytes() as i64
    )
    if not loaded.ok then
        return fail("decode-frame", "read", 1, 66)
    end if
    let trace = format::decode_trace(loaded.data)
    if format::trace_status_code(trace.status) != 0 then
        return fail("decode-frame", "container", format::trace_status_code(trace.status), 67)
    end if
    if selected.value >= trace.frame_count then
        return fail("decode-frame", "index", 2, 65)
    end if
    let pixels = format::frame_pixels(trace, selected.value)
    let document = ppm::encode_ppm(trace.width, trace.height, pixels)
    if ppm::ppm_status_code(document.status) != 0 then
        return fail("decode-frame", "ppm", ppm::ppm_status_code(document.status), 67)
    end if
    if not write_atomic(ny::lang::system::arg(3), document.data, false) then
        return fail("decode-frame", "publish", 1, 68)
    end if

    println("operation=decode-frame")
    println("status=ok")
    print_i32_field("frame", selected.value)
    print_i32_field("width", trace.width)
    print_i32_field("height", trace.height)
    print_i32_field("bytes", document.data.len())
    print_i64_field("pixel_crc32c", codec::crc32c(pixels))
    return 0
end fn

fn main() -> i32
    if ny::lang::system::arg_count() < 1 then
        return usage()
    end if
    let command = ny::lang::system::arg(0)
    if command == "encode" then
        return encode_command()
    elseif command == "inspect" then
        return inspect_command()
    elseif command == "decode-frame" then
        return decode_frame_command()
    end if
    return usage()
end fn
