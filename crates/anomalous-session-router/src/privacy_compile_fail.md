
## Compile-fail authority and privacy matrix

The router's control, source, focus, prepared-action, rejection, receipt, and
owner types remain move-only and non-formatting. Each block independently makes
an accidental trait addition fail this crate's documentation tests.

### `router_control`

```compile_fail
use anomalous_session_router::router_control;
fn require<T: Clone>() {}
require::<router_control>();
```

```compile_fail
use anomalous_session_router::router_control;
fn require<T: core::fmt::Debug>() {}
require::<router_control>();
```

```compile_fail
use anomalous_session_router::router_control;
fn require<T: core::fmt::Display>() {}
require::<router_control>();
```

```compile_fail
use anomalous_session_router::router_control;
fn require<T: core::hash::Hash>() {}
require::<router_control>();
```

### `host_verified_source`

```compile_fail
use anomalous_session_router::host_verified_source;
fn require<T: Clone>() {}
require::<host_verified_source>();
```

```compile_fail
use anomalous_session_router::host_verified_source;
fn require<T: core::fmt::Debug>() {}
require::<host_verified_source>();
```

```compile_fail
use anomalous_session_router::host_verified_source;
fn require<T: core::fmt::Display>() {}
require::<host_verified_source>();
```

```compile_fail
use anomalous_session_router::host_verified_source;
fn require<T: core::hash::Hash>() {}
require::<host_verified_source>();
```

### `focus_authority`

```compile_fail
use anomalous_session_router::focus_authority;
fn require<T: Clone>() {}
require::<focus_authority>();
```

```compile_fail
use anomalous_session_router::focus_authority;
fn require<T: core::fmt::Debug>() {}
require::<focus_authority>();
```

```compile_fail
use anomalous_session_router::focus_authority;
fn require<T: core::fmt::Display>() {}
require::<focus_authority>();
```

```compile_fail
use anomalous_session_router::focus_authority;
fn require<T: core::hash::Hash>() {}
require::<focus_authority>();
```

### `focus_change_rejection`

```compile_fail
use anomalous_session_router::focus_change_rejection;
fn require<T: Clone>() {}
require::<focus_change_rejection>();
```

```compile_fail
use anomalous_session_router::focus_change_rejection;
fn require<T: core::fmt::Debug>() {}
require::<focus_change_rejection>();
```

```compile_fail
use anomalous_session_router::focus_change_rejection;
fn require<T: core::fmt::Display>() {}
require::<focus_change_rejection>();
```

```compile_fail
use anomalous_session_router::focus_change_rejection;
fn require<T: core::hash::Hash>() {}
require::<focus_change_rejection>();
```

### `focus_transfer`

```compile_fail
use anomalous_session_router::focus_transfer;
fn require<T: Clone>() {}
require::<focus_transfer>();
```

```compile_fail
use anomalous_session_router::focus_transfer;
fn require<T: core::fmt::Debug>() {}
require::<focus_transfer>();
```

```compile_fail
use anomalous_session_router::focus_transfer;
fn require<T: core::fmt::Display>() {}
require::<focus_transfer>();
```

```compile_fail
use anomalous_session_router::focus_transfer;
fn require<T: core::hash::Hash>() {}
require::<focus_transfer>();
```

### `focus_revocation`

```compile_fail
use anomalous_session_router::focus_revocation;
fn require<T: Clone>() {}
require::<focus_revocation>();
```

```compile_fail
use anomalous_session_router::focus_revocation;
fn require<T: core::fmt::Debug>() {}
require::<focus_revocation>();
```

```compile_fail
use anomalous_session_router::focus_revocation;
fn require<T: core::fmt::Display>() {}
require::<focus_revocation>();
```

```compile_fail
use anomalous_session_router::focus_revocation;
fn require<T: core::hash::Hash>() {}
require::<focus_revocation>();
```

### `prepared_routed_submission`

```compile_fail
use anomalous_session_router::prepared_routed_submission;
fn require<T: Clone>() {}
require::<prepared_routed_submission>();
```

```compile_fail
use anomalous_session_router::prepared_routed_submission;
fn require<T: core::fmt::Debug>() {}
require::<prepared_routed_submission>();
```

```compile_fail
use anomalous_session_router::prepared_routed_submission;
fn require<T: core::fmt::Display>() {}
require::<prepared_routed_submission>();
```

```compile_fail
use anomalous_session_router::prepared_routed_submission;
fn require<T: core::hash::Hash>() {}
require::<prepared_routed_submission>();
```

### `route_prepare_rejection`

```compile_fail
use anomalous_session_router::route_prepare_rejection;
fn require<T: Clone>() {}
require::<route_prepare_rejection>();
```

```compile_fail
use anomalous_session_router::route_prepare_rejection;
fn require<T: core::fmt::Debug>() {}
require::<route_prepare_rejection>();
```

```compile_fail
use anomalous_session_router::route_prepare_rejection;
fn require<T: core::fmt::Display>() {}
require::<route_prepare_rejection>();
```

```compile_fail
use anomalous_session_router::route_prepare_rejection;
fn require<T: core::hash::Hash>() {}
require::<route_prepare_rejection>();
```

### `route_publish_rejection`

```compile_fail
use anomalous_session_router::route_publish_rejection;
fn require<T: Clone>() {}
require::<route_publish_rejection>();
```

```compile_fail
use anomalous_session_router::route_publish_rejection;
fn require<T: core::fmt::Debug>() {}
require::<route_publish_rejection>();
```

```compile_fail
use anomalous_session_router::route_publish_rejection;
fn require<T: core::fmt::Display>() {}
require::<route_publish_rejection>();
```

```compile_fail
use anomalous_session_router::route_publish_rejection;
fn require<T: core::hash::Hash>() {}
require::<route_publish_rejection>();
```

### `routed_publish_receipt`

```compile_fail
use anomalous_session_router::routed_publish_receipt;
fn require<T: Clone>() {}
require::<routed_publish_receipt>();
```

```compile_fail
use anomalous_session_router::routed_publish_receipt;
fn require<T: core::fmt::Debug>() {}
require::<routed_publish_receipt>();
```

```compile_fail
use anomalous_session_router::routed_publish_receipt;
fn require<T: core::fmt::Display>() {}
require::<routed_publish_receipt>();
```

```compile_fail
use anomalous_session_router::routed_publish_receipt;
fn require<T: core::hash::Hash>() {}
require::<routed_publish_receipt>();
```

### `session_router`

```compile_fail
use anomalous_session_router::session_router;
fn require<T: Clone>() {}
require::<session_router>();
```

```compile_fail
use anomalous_session_router::session_router;
fn require<T: core::fmt::Debug>() {}
require::<session_router>();
```

```compile_fail
use anomalous_session_router::session_router;
fn require<T: core::fmt::Display>() {}
require::<session_router>();
```

```compile_fail
use anomalous_session_router::session_router;
fn require<T: core::hash::Hash>() {}
require::<session_router>();
```

### Authority construction stays private

Copyable scalar identities cannot fabricate router control:

```compile_fail
use anomalous_session_router::{router_control, router_identity};

fn fabricate(router: router_identity) -> router_control {
    router_control { router }
}
```

They cannot fabricate a source authority:

```compile_fail
use anomalous_session_router::{
    host_verified_source, principal_identity, router_identity, source_identity, source_kind,
};

fn fabricate(
    router: router_identity,
    identity: source_identity,
    principal: principal_identity,
) -> host_verified_source {
    host_verified_source {
        router,
        identity,
        principal,
        kind: source_kind::human_focus,
    }
}
```
