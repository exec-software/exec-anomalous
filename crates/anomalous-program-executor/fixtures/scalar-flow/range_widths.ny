module anomalous.scalar_flow.range_widths

@ai_context("Check hidden integer range counters remain wide while exposed indices narrow")
fn main() -> i32
    let mut total: i64 = 0
    for boundary_index in range(2147483647, 2147483649)
        println(boundary_index)
        total = total + boundary_index
    end for
    for upper_index in range(9223372036854775806, 9223372036854775807)
        println(upper_index)
        total = total + upper_index
    end for
    return total
end fn
