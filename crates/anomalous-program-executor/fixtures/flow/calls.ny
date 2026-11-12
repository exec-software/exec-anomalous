module anomalous.flow.calls

fn wrapper(value: i64) -> i64
    let saved: i64 = 100
    let inner: i64 = difference(value, 3)
    return saved + inner
end fn

@ai_context("Check ordered direct arguments and isolated nested call frames")
fn main() -> i32
    return wrapper(9) + difference(9, 4) - 69
end fn

fn difference(left: i64, right: i64) -> i64
    return left - right
end fn
