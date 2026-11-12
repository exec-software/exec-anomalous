module anomalous.scalar_flow.bounds_once

fn bound(value: i64) -> i64
    println(value)
    return value
end fn

@ai_context("Check integer range bounds evaluate left to right exactly once")
fn main() -> i32
    let mut total: i64 = 0
    for index in range(bound(2), bound(5))
        total = total + index
    end for
    return total
end fn
