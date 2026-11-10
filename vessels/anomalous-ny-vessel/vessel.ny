module anomalous_ny_vessel

# A Ny vessel that issues its own rites, and now hears an answer to one.
#
# The console write and the input read are symbols the Anomalous half of this
# vessel defines and Ny declares with @interop("host"). Nothing of the rite ABI
# is in Ny: it sees a function that takes text and returns a count, and one
# that takes nothing and returns text. That is the whole point -- the ABI is
# ours and unstable, so what the toolchain supplies is the ability to reach it,
# not bindings to it.
#
# The vessel composes its message rather than naming a constant, because
# composition is what needs the allocator the host installed, and the bytes
# land in the host's region. It then reads the rite's result back and only
# speaks a second time if the first write committed what it sent, so the
# vessel is acting on the ABI rather than merely crossing it.
#
# The third write is the one that needed a second operation: the text in it
# came from outside the vessel. Ny asks for a line, is given one it could not
# have contained, composes an answer around it in the region, and speaks that.
# Data now travels into Ny through a rite and back out through another.
#
# One run of this vessel is meant to fail. The host asks for it through a cell
# rather than a rite, and the vessel answers by exhausting the region on
# purpose, so what Ny does when an allocation cannot be satisfied is something
# the kernel observes rather than something a comment claims.

@interop("host")
fn anomalous_rite_console_write(message: text) -> i64
end fn

@interop("host")
fn anomalous_rite_input_read() -> text
end fn

# Not a rite: the host writes a cell before the vessel runs and this reads it.
# It is how one image can be asked for the run that is meant to fail.
@interop("host")
fn anomalous_vessel_mode() -> i64
end fn

let* vessel_message: text = ""
let* vessel_scratch: text = ""

fn main() -> i64
    let mode: i64 = anomalous_vessel_mode()
    if mode != 0 then
        # The run the host asked to fail. Concatenating without bound is the
        # one failure a vessel can reach deliberately: the region it was given
        # runs out, the allocation returns nothing, and the runtime's failure
        # hook takes it from there. Nothing below this line executes.
        let seed: text = "0123456789"
        vessel_scratch = seed
        let mut index: i64 = 0
        while index < 400 do
            vessel_scratch = vessel_scratch + seed
            index = index + 1
        end while
        return 1
    end if
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
    let heard: text = anomalous_rite_input_read()
    let preface: text = "ny heard: "
    let answer: text = preface + heard
    let spoken: i64 = anomalous_rite_console_write(answer)
    if spoken != 21 then
        return 7
    end if
    return 42
end fn
