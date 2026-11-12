module anomalous.scalar_flow.zero_ranges

@ai_context("Check empty one argument equal endpoint and reversed integer ranges")
fn main() -> i32
    let mut total: i64 = 7
    for first in range(0)
        total = total + 100
    end for
    for second in range(4, 4)
        total = total + 1000
    end for
    for third in range(5, 2)
        total = total + 10000
    end for
    for fourth in range(-3)
        total = total + 100000
    end for
    return total
end fn
