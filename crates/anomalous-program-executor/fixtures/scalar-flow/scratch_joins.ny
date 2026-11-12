module anomalous.scalar_flow.scratch_joins

fn classify(value: i64) -> i64
    let selected: i64 = if (value > 0 and value < 3) or value == 5 then 10 else 20 end if
    return selected
end fn

@ai_context("Check generated scratch slots remain separate from authored locals and other frames")
fn main() -> i32
    let idx_0: i64 = 70
    let len_1: i64 = 80
    let logical_and_0: i64 = 90
    let logical_or_1: i64 = 100
    let mut total: i64 = 0
    for index in range(6)
        total = total + classify(index)
    end for
    return total + idx_0 + len_1 + logical_and_0 + logical_or_1
end fn
