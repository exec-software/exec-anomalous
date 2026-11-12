module anomalous.flow.branches

fn choose(value: i64) -> i64
    let mut result: i64 = 9
    if value > 0 then
        result = 7
    elseif value < 0 then
        result = 11
    end if
    return result
end fn

@ai_context("Check integer branch polarity and initialized local joins")
fn main() -> i32
    let selected: i64 = if 3 > 0 then 5 else 8 end if
    return choose(-1) + choose(0) + choose(1) + selected
end fn
