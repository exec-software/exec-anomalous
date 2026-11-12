module anomalous.scalar_flow.fuel

@ai_context("Check finite integer range execution stops at the candidate fuel bound")
fn main() -> i32
    let mut total: i64 = 0
    for index in range(10000)
        total = total + 1
    end for
    return total
end fn
