module anomalous.flow.while_case

fn sum_until(limit: i64) -> i64
    let mut index: i64 = 0
    let mut total: i64 = 0
    while index < limit
        index = index + 1
        if index == 2 then
            continue
        end if
        if index == 6 then
            break
        end if
        total = total + index
    end while
    return total
end fn

@ai_context("Check bounded while execution with zero iterations continue and break")
fn main() -> i32
    return sum_until(0) + sum_until(10)
end fn
