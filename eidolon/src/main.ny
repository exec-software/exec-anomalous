module eidolon

import codec

@ai_context(
    intent="Exercise Eidolon's portable in-memory codec through checked NIL",
    assumptions=["The Python reference remains the independent draft-1 oracle"],
    constraints=[
        "Emit exact machine-checkable octets",
        "Never substitute the Python implementation for a Ny result",
        "Keep capture policy outside the codec"
    ]
)
fn main() -> i32
    return codec::run_conformance()
end fn
