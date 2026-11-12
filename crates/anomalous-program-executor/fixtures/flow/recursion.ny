module anomalous.flow.recursion

fn factorial(count: i64) -> i64
    if count <= 1 then
        return 1
    end if
    return count * factorial(count - 1)
end fn

@ai_context("Check bounded recursion and restoration of caller operand stacks")
fn main() -> i32
    return factorial(5)
end fn
