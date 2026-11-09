module anomalous_ny_vessel

# A Ny vessel that issues its own rites.
#
# The console write is a symbol the Anomalous half of this vessel defines and
# Ny declares with @interop("host"). Nothing of the rite ABI is in Ny: it sees
# a function that takes text and returns a count. That is the whole point --
# the ABI is ours and unstable, so what the toolchain supplies is the ability
# to reach it, not bindings to it.
#
# The vessel composes its message rather than naming a constant, because
# composition is what needs the allocator the host installed, and the bytes
# land in the host's region. It then reads the rite's result back and only
# speaks a second time if the first write committed what it sent, so the
# vessel is acting on the ABI rather than merely crossing it.

@interop("host")
fn anomalous_rite_console_write(message: text) -> i64
end fn

let* vessel_message: text = ""

fn main() -> i64
    let subject: text = "Anomalous"
    let predicate: text = " vessel speaks through a rite"
    vessel_message = subject + predicate
    let written: i64 = anomalous_rite_console_write(vessel_message)
    if written != 38 then
        return 9
    end if
    let confirmation: text = "ny saw the rite succeed"
    let echoed: i64 = anomalous_rite_console_write(confirmation)
    if echoed != 23 then
        return 8
    end if
    return 42
end fn
