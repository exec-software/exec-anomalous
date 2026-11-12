module anomalous.flow.text_void

fn choose(value: i64) -> text
    if value > 0 then
        return "yes"
    end if
    return "no"
end fn

fn echo(message: text) -> text
    return message
end fn

fn announce(message: text) -> void
    println(message)
end fn

@ai_context("Check borrowed text results and explicit void console calls")
fn main() -> i32
    announce(echo(choose(1)))
    return 0
end fn
