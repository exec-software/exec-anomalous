module anomalous.scalar_flow.nested_ranges

@ai_context("Check independent hidden counters for nested integer ranges")
fn main() -> i32
    let mut total: i64 = 0
    for outer in range(1, 4)
        for inner in range(3)
            total = total + outer * 10 + inner
        end for
    end for
    return total
end fn
