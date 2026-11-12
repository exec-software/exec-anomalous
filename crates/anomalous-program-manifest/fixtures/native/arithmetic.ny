module anomalous.manifest.arithmetic

@ai_context("Calculate a bounded integer result without external services")
fn main() -> i32
    let left: i64 = 6
    let right: i64 = 7
    let answer: i64 = left * right
    return answer as i32
end fn
