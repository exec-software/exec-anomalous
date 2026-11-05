
## Compile-fail privacy and ownership matrix

Each block isolates one type/trait pair so adding any implicit clone,
formatting, or hashing surface turns that regression's block into an
unexpected success.

### `private_text`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::private_text;

fn require_trait<T: Clone>() {}
require_trait::<private_text>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::private_text;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<private_text>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::private_text;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<private_text>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::private_text;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<private_text>();
```

### `semantic_action`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::semantic_action;

fn require_trait<T: Clone>() {}
require_trait::<semantic_action>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::semantic_action;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<semantic_action>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::semantic_action;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<semantic_action>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::semantic_action;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<semantic_action>();
```

### `prepared_submission`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::prepared_submission;

fn require_trait<T: Clone>() {}
require_trait::<prepared_submission>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::prepared_submission;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<prepared_submission>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::prepared_submission;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<prepared_submission>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::prepared_submission;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<prepared_submission>();
```

### `prepare_rejection`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::prepare_rejection;

fn require_trait<T: Clone>() {}
require_trait::<prepare_rejection>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::prepare_rejection;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<prepare_rejection>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::prepare_rejection;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<prepare_rejection>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::prepare_rejection;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<prepare_rejection>();
```

### `publish_rejection`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::publish_rejection;

fn require_trait<T: Clone>() {}
require_trait::<publish_rejection>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::publish_rejection;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<publish_rejection>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::publish_rejection;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<publish_rejection>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::publish_rejection;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<publish_rejection>();
```

### `publish_receipt`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::publish_receipt;

fn require_trait<T: Clone>() {}
require_trait::<publish_receipt>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::publish_receipt;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<publish_receipt>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::publish_receipt;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<publish_receipt>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::publish_receipt;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<publish_receipt>();
```

### `session_submission`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::session_submission;

fn require_trait<T: Clone>() {}
require_trait::<session_submission>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::session_submission;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<session_submission>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::session_submission;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<session_submission>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::session_submission;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<session_submission>();
```

### `prepared_wait`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::prepared_wait;

fn require_trait<T: Clone>() {}
require_trait::<prepared_wait>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::prepared_wait;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<prepared_wait>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::prepared_wait;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<prepared_wait>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::prepared_wait;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<prepared_wait>();
```

### `receive_preparation`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::receive_preparation;

fn require_trait<T: Clone>() {}
require_trait::<receive_preparation>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::receive_preparation;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<receive_preparation>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::receive_preparation;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<receive_preparation>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::receive_preparation;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<receive_preparation>();
```

### `wait_commit`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::wait_commit;

fn require_trait<T: Clone>() {}
require_trait::<wait_commit>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::wait_commit;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<wait_commit>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::wait_commit;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<wait_commit>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::wait_commit;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<wait_commit>();
```

### `wake_acknowledgement`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::wake_acknowledgement;

fn require_trait<T: Clone>() {}
require_trait::<wake_acknowledgement>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::wake_acknowledgement;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<wake_acknowledgement>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::wake_acknowledgement;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<wake_acknowledgement>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::wake_acknowledgement;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<wake_acknowledgement>();
```

### `wait_cancellation`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::wait_cancellation;

fn require_trait<T: Clone>() {}
require_trait::<wait_cancellation>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::wait_cancellation;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<wait_cancellation>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::wait_cancellation;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<wait_cancellation>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::wait_cancellation;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<wait_cancellation>();
```

### `close_receipt`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::close_receipt;

fn require_trait<T: Clone>() {}
require_trait::<close_receipt>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::close_receipt;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<close_receipt>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::close_receipt;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<close_receipt>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::close_receipt;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<close_receipt>();
```

### `shell_session`

`Clone` remains unavailable:

```compile_fail
use anomalous_session::shell_session;

fn require_trait<T: Clone>() {}
require_trait::<shell_session>();
```

`Debug` remains unavailable:

```compile_fail
use anomalous_session::shell_session;

fn require_trait<T: core::fmt::Debug>() {}
require_trait::<shell_session>();
```

`Display` remains unavailable:

```compile_fail
use anomalous_session::shell_session;

fn require_trait<T: core::fmt::Display>() {}
require_trait::<shell_session>();
```

`Hash` remains unavailable:

```compile_fail
use anomalous_session::shell_session;

fn require_trait<T: core::hash::Hash>() {}
require_trait::<shell_session>();
```
