module anomalous.scalar_flow.break_continue

@ai_context("Check range continue increments and break skips the remaining iterations")
fn main() -> i32
    let mut total: i64 = 0
    for index in range(0, 10)
        if index == 2 then
            continue
        end if
        if index == 6 then
            break
        end if
        println(index)
        total = total + index
    end for
    return total
end fn
