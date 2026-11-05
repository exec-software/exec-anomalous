module eidolon.tests.format_timing_probe

import codec
import format
import ppm
import ny.lang.core.*
import ny.lang.io.console
import ny.lang.io.file
import ny.lang.system.std_sys

fn wire_bytes(values: array<i32>) -> array<u8>
    let mut output: array<u8> = []
    let mut index: i32 = 0
    while index < values.len()
        output.push(values[index] as u8)
        index = index + 1
    end while
    return output
end fn

fn publish(path: text, values: array<i32>) -> bool
    let result = ny::lang::io::write_bytes_atomic(path, wire_bytes(values), false)
    match result
        Result.Ok(written) -> return written
        Result.Error(_) -> return false
    end match
end fn

fn report(name: text, value: i32)
    println(name + "=" + Convert.i32_to_text(value))
end fn

fn main() -> i32
    if ny::lang::system::arg_count() != 1 then
        return 64
    end if
    let pixels: array<i32> = [0, 1, 2, 255, 128, 64]
    let encoded = format::encode_trace(
        pixels,
        2,
        1,
        1,
        1,
        1,
        4294967295,
        2147483648,
        4294967295,
        120
    )
    let empty_pixels: array<i32> = []
    let payload_capacity_status = format::encode_trace(
        empty_pixels, 1, 513, 256, 1, 1, 1, 1, 1, 120
    )
    let maximum_ppm_extent = format::maximum_frame_bytes() +
        ppm::maximum_header_bytes()
    let ppm_extent_at_limit = ppm::input_extent_status(maximum_ppm_extent)
    let ppm_extent_over_limit = ppm::input_extent_status(maximum_ppm_extent + 1)
    report("status", format::trace_status_code(encoded.status))
    report("maximum_frame_bytes", format::maximum_frame_bytes())
    report("maximum_payload_bytes", format::maximum_payload_bytes())
    report("maximum_container_bytes", format::maximum_container_bytes())
    report(
        "maximum_total_decoded_bytes",
        format::maximum_total_decoded_bytes()
    )
    report(
        "payload_capacity_status",
        format::trace_status_code(payload_capacity_status.status)
    )
    report(
        "ppm_input_extent_at_limit_status",
        ppm::ppm_status_code(ppm_extent_at_limit)
    )
    report(
        "ppm_input_extent_over_limit_status",
        ppm::ppm_status_code(ppm_extent_over_limit)
    )
    if format::trace_status_code(encoded.status) != 0 then
        return 1
    end if
    if format::trace_status_code(payload_capacity_status.status) != 23 then
        return 1
    end if
    if ppm::ppm_status_code(ppm_extent_at_limit) != 0 ||
        ppm::ppm_status_code(ppm_extent_over_limit) != 8 then
        return 1
    end if
    if not publish(ny::lang::system::arg(0), encoded.data) then
        return 2
    end if

    let invalid_timebase_low = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 0, 1, 1, 120
    )
    let invalid_timebase_high = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 4294967296, 1, 1, 120
    )
    let invalid_denominator_low = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 1, 0, 1, 120
    )
    let invalid_denominator_high = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 1, 4294967296, 1, 120
    )
    let invalid_duration_low = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 1, 1, 0, 120
    )
    let invalid_duration_high = format::encode_trace(
        pixels, 2, 1, 1, 1, 1, 1, 1, 4294967296, 120
    )
    report(
        "invalid_timebase_low",
        format::trace_status_code(invalid_timebase_low.status)
    )
    report(
        "invalid_timebase_high",
        format::trace_status_code(invalid_timebase_high.status)
    )
    report(
        "invalid_denominator_low",
        format::trace_status_code(invalid_denominator_low.status)
    )
    report(
        "invalid_denominator_high",
        format::trace_status_code(invalid_denominator_high.status)
    )
    report(
        "invalid_duration_low",
        format::trace_status_code(invalid_duration_low.status)
    )
    report(
        "invalid_duration_high",
        format::trace_status_code(invalid_duration_high.status)
    )
    return 0
end fn
