module anomalous.flow.early_returns

fn sign(value: i64) -> i64
    if value > 0 then
        return 1
    else
        return -1
    end if
end fn

@ai_context("Check early returns and unreachable compiler fallthrough markers")
fn main() -> i32
    return sign(0) + sign(1)
end fn
