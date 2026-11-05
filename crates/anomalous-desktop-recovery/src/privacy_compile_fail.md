## Local authority and privacy boundaries

Every owner remains move-only, non-formatting, and non-hashable. These are
independent compile-fail checks so one prohibited trait cannot hide another.
Public status and error codes are descriptive and cannot reconstruct owners.

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn require<T: Clone>() {}
require::<recovery_surface>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn require<T: core::fmt::Debug>() {}
require::<recovery_surface>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn require<T: core::fmt::Display>() {}
require::<recovery_surface>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn require<T: core::hash::Hash>() {}
require::<recovery_surface>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_control;
fn require<T: Clone>() {}
require::<recovery_control>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_control;
fn require<T: core::fmt::Debug>() {}
require::<recovery_control>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_control;
fn require<T: core::fmt::Display>() {}
require::<recovery_control>();
```

```compile_fail
use anomalous_desktop_recovery::recovery_control;
fn require<T: core::hash::Hash>() {}
require::<recovery_control>();
```

```compile_fail
use anomalous_desktop_recovery::presenter_endpoint;
fn require<T: Clone>() {}
require::<presenter_endpoint>();
```

```compile_fail
use anomalous_desktop_recovery::presenter_endpoint;
fn require<T: core::fmt::Debug>() {}
require::<presenter_endpoint>();
```

```compile_fail
use anomalous_desktop_recovery::presenter_endpoint;
fn require<T: core::fmt::Display>() {}
require::<presenter_endpoint>();
```

```compile_fail
use anomalous_desktop_recovery::presenter_endpoint;
fn require<T: core::hash::Hash>() {}
require::<presenter_endpoint>();
```

```compile_fail
use anomalous_desktop_recovery::presentation_ticket;
fn require<T: Clone>() {}
require::<presentation_ticket>();
```

```compile_fail
use anomalous_desktop_recovery::presentation_ticket;
fn require<T: core::fmt::Debug>() {}
require::<presentation_ticket>();
```

```compile_fail
use anomalous_desktop_recovery::presentation_ticket;
fn require<T: core::fmt::Display>() {}
require::<presentation_ticket>();
```

```compile_fail
use anomalous_desktop_recovery::presentation_ticket;
fn require<T: core::hash::Hash>() {}
require::<presentation_ticket>();
```

```compile_fail
use anomalous_desktop_recovery::construction_rejection;
fn require<T: Clone>() {}
require::<construction_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::construction_rejection;
fn require<T: core::fmt::Debug>() {}
require::<construction_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::construction_rejection;
fn require<T: core::fmt::Display>() {}
require::<construction_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::construction_rejection;
fn require<T: core::hash::Hash>() {}
require::<construction_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::submission_rejection;
fn require<T: Clone>() {}
require::<submission_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::submission_rejection;
fn require<T: core::fmt::Debug>() {}
require::<submission_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::submission_rejection;
fn require<T: core::fmt::Display>() {}
require::<submission_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::submission_rejection;
fn require<T: core::hash::Hash>() {}
require::<submission_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::finish_rejection;
fn require<T: Clone>() {}
require::<finish_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::finish_rejection;
fn require<T: core::fmt::Debug>() {}
require::<finish_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::finish_rejection;
fn require<T: core::fmt::Display>() {}
require::<finish_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::finish_rejection;
fn require<T: core::hash::Hash>() {}
require::<finish_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::retirement_rejection;
fn require<T: Clone>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::retirement_rejection;
fn require<T: core::fmt::Debug>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::retirement_rejection;
fn require<T: core::fmt::Display>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_recovery::retirement_rejection;
fn require<T: core::hash::Hash>() {}
require::<retirement_rejection>();
```

The coordinator exposes no inner service or publication controls.

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn escape(owner: &mut recovery_surface) {
    let _ = &mut owner.surface;
}
```

```compile_fail
use anomalous_desktop_recovery::recovery_surface;
fn escape(owner: &recovery_surface) {
    let _ = &owner.presentation;
}
```

Tickets and endpoints cannot be reconstructed from numeric correlation data.

```compile_fail
use anomalous_desktop_recovery::presentation_ticket;
let forged = presentation_ticket {
    instance: 1,
    presenter_generation: 1,
    transaction: 1,
};
```

```compile_fail
use anomalous_desktop_recovery::presenter_endpoint;
let forged = presenter_endpoint { instance: 1, generation: 1 };
```

A borrowed semantic view cannot be held through a mutable begin transition.

```compile_fail
use anomalous_desktop_recovery::{recovery_surface, presenter_endpoint};
fn retain_view(owner: &mut recovery_surface, endpoint: &presenter_endpoint) {
    let view = owner.presented().unwrap();
    let _ticket = owner.begin(endpoint).unwrap();
    let _ = view.revision();
}
```
