module anomalous_ny_probe

let* initialized_value: i64 = 37
let* zero_value: i64 = 0
let* floating_result: f64 = 0.0

fn calculate(seed: i64) -> i32
    let mut count: i64 = 0
    let mut total: f64 = seed as f64
    while count < 50000000
        total = total + 0.5
        count = count + 1
    end while
    zero_value = count
    floating_result = total
    if floating_result == 25000037.0 and zero_value == 50000000 then
        return zero_value as i32
    end if
    return 9
end fn

fn main() -> i32
    if initialized_value != 37 or zero_value != 0 or floating_result != 0.0 then
        return 8
    end if
    return calculate(initialized_value)
end fn
