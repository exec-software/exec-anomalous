module anomalous.scalar_flow.short_circuit

fn report(value: i64) -> i64
    println(value)
    return value
end fn

fn broken() -> i64
    let zero: i64 = 0
    return 1 / zero
end fn

@ai_context("Check logical scratch joins preserve short circuit output and skipped faults")
fn main() -> i32
    let mut total: i64 = 0
    if false and report(11) > 0 then
        total = total + 100
    end if
    if true or report(22) > 0 then
        total = total + 1
    end if
    if true and report(33) > 0 then
        total = total + 2
    end if
    if false or report(44) > 0 then
        total = total + 4
    end if
    if false and broken() > 0 then
        total = total + 1000
    end if
    if true or broken() > 0 then
        total = total + 8
    end if
    return total
end fn
