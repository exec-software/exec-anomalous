module anomalous_ny_vessel

# The first Ny vessel built on the shipped freestanding ABI rather than on an
# inspected entry shape. It composes its own message instead of naming a
# constant, because composition is what needs the allocator the host installs:
# the bytes are written into the region the host owns, which is how they reach
# the rite without Ny ever learning a kernel address.
#
# The module global keeps the composed text live. Nothing here is exported --
# a Ny object publishes only __ny_entry -- so the host reads the message from
# the region it installed, at its base, as the vessel's first allocation.

let* vessel_message: text = ""

fn main() -> i64
    let subject: text = "Anomalous"
    let predicate: text = " vessel speaks through a rite"
    vessel_message = subject + predicate
    return 42
end fn
