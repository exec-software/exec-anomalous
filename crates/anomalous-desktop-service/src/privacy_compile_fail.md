## Compile-fail authority and privacy matrix

The service owner, controls, renderer endpoint, transition owners, and branded
action/focus handles remain move-only and non-formatting. Rejection owners also
remain move-only. Independent blocks make accidental trait additions fail
documentation tests.

### Core controls

```compile_fail
use anomalous_desktop_service::surface_control;
fn require<T: Clone>() {}
require::<surface_control>();
```

```compile_fail
use anomalous_desktop_service::surface_control;
fn require<T: core::fmt::Debug>() {}
require::<surface_control>();
```

```compile_fail
use anomalous_desktop_service::surface_control;
fn require<T: core::fmt::Display>() {}
require::<surface_control>();
```

```compile_fail
use anomalous_desktop_service::surface_control;
fn require<T: core::hash::Hash>() {}
require::<surface_control>();
```

```compile_fail
use anomalous_desktop_service::presentation_control;
fn require<T: Clone>() {}
require::<presentation_control>();
```

```compile_fail
use anomalous_desktop_service::presentation_control;
fn require<T: core::fmt::Debug>() {}
require::<presentation_control>();
```

```compile_fail
use anomalous_desktop_service::presentation_control;
fn require<T: core::fmt::Display>() {}
require::<presentation_control>();
```

```compile_fail
use anomalous_desktop_service::presentation_control;
fn require<T: core::hash::Hash>() {}
require::<presentation_control>();
```

```compile_fail
use anomalous_desktop_service::renderer_endpoint;
fn require<T: Clone>() {}
require::<renderer_endpoint>();
```

```compile_fail
use anomalous_desktop_service::renderer_endpoint;
fn require<T: core::fmt::Debug>() {}
require::<renderer_endpoint>();
```

```compile_fail
use anomalous_desktop_service::renderer_endpoint;
fn require<T: core::fmt::Display>() {}
require::<renderer_endpoint>();
```

```compile_fail
use anomalous_desktop_service::renderer_endpoint;
fn require<T: core::hash::Hash>() {}
require::<renderer_endpoint>();
```

### Publication owners

```compile_fail
use anomalous_desktop_service::render_lease;
fn require<T: Clone>() {}
require::<render_lease>();
```

```compile_fail
use anomalous_desktop_service::render_lease;
fn require<T: core::fmt::Debug>() {}
require::<render_lease>();
```

```compile_fail
use anomalous_desktop_service::render_lease;
fn require<T: core::fmt::Display>() {}
require::<render_lease>();
```

```compile_fail
use anomalous_desktop_service::render_lease;
fn require<T: core::hash::Hash>() {}
require::<render_lease>();
```

```compile_fail
use anomalous_desktop_service::prepared_presentation;
fn require<T: Clone>() {}
require::<prepared_presentation>();
```

```compile_fail
use anomalous_desktop_service::prepared_presentation;
fn require<T: core::fmt::Debug>() {}
require::<prepared_presentation>();
```

```compile_fail
use anomalous_desktop_service::prepared_presentation;
fn require<T: core::fmt::Display>() {}
require::<prepared_presentation>();
```

```compile_fail
use anomalous_desktop_service::prepared_presentation;
fn require<T: core::hash::Hash>() {}
require::<prepared_presentation>();
```

```compile_fail
use anomalous_desktop_service::presentation_receipt;
fn require<T: Clone>() {}
require::<presentation_receipt>();
```

```compile_fail
use anomalous_desktop_service::presentation_receipt;
fn require<T: core::fmt::Debug>() {}
require::<presentation_receipt>();
```

```compile_fail
use anomalous_desktop_service::presentation_receipt;
fn require<T: core::fmt::Display>() {}
require::<presentation_receipt>();
```

```compile_fail
use anomalous_desktop_service::presentation_receipt;
fn require<T: core::hash::Hash>() {}
require::<presentation_receipt>();
```

```compile_fail
use anomalous_desktop_service::queued_revision;
fn require<T: Clone>() {}
require::<queued_revision>();
```

```compile_fail
use anomalous_desktop_service::queued_revision;
fn require<T: core::fmt::Debug>() {}
require::<queued_revision>();
```

```compile_fail
use anomalous_desktop_service::queued_revision;
fn require<T: core::fmt::Display>() {}
require::<queued_revision>();
```

```compile_fail
use anomalous_desktop_service::queued_revision;
fn require<T: core::hash::Hash>() {}
require::<queued_revision>();
```

```compile_fail
use anomalous_desktop_service::publication_receipt;
fn require<T: Clone>() {}
require::<publication_receipt>();
```

```compile_fail
use anomalous_desktop_service::publication_receipt;
fn require<T: core::fmt::Debug>() {}
require::<publication_receipt>();
```

```compile_fail
use anomalous_desktop_service::publication_receipt;
fn require<T: core::fmt::Display>() {}
require::<publication_receipt>();
```

```compile_fail
use anomalous_desktop_service::publication_receipt;
fn require<T: core::hash::Hash>() {}
require::<publication_receipt>();
```

```compile_fail
use anomalous_desktop_service::restart_receipt;
fn require<T: Clone>() {}
require::<restart_receipt>();
```

```compile_fail
use anomalous_desktop_service::restart_receipt;
fn require<T: core::fmt::Debug>() {}
require::<restart_receipt>();
```

```compile_fail
use anomalous_desktop_service::restart_receipt;
fn require<T: core::fmt::Display>() {}
require::<restart_receipt>();
```

```compile_fail
use anomalous_desktop_service::restart_receipt;
fn require<T: core::hash::Hash>() {}
require::<restart_receipt>();
```

### Local semantic capabilities

```compile_fail
use anomalous_desktop_service::surface_action_handle;
fn require<T: Clone>() {}
require::<surface_action_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_action_handle;
fn require<T: core::fmt::Debug>() {}
require::<surface_action_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_action_handle;
fn require<T: core::fmt::Display>() {}
require::<surface_action_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_action_handle;
fn require<T: core::hash::Hash>() {}
require::<surface_action_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_handle;
fn require<T: Clone>() {}
require::<surface_focus_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_handle;
fn require<T: core::fmt::Debug>() {}
require::<surface_focus_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_handle;
fn require<T: core::fmt::Display>() {}
require::<surface_focus_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_handle;
fn require<T: core::hash::Hash>() {}
require::<surface_focus_handle>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_resolution;
fn require<T: Clone>() {}
require::<surface_focus_resolution>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_resolution;
fn require<T: core::fmt::Debug>() {}
require::<surface_focus_resolution>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_resolution;
fn require<T: core::fmt::Display>() {}
require::<surface_focus_resolution>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_resolution;
fn require<T: core::hash::Hash>() {}
require::<surface_focus_resolution>();
```

### Owners and rejection paths

```compile_fail
use anomalous_desktop_service::desktop_surface;
fn require<T: Clone>() {}
require::<desktop_surface>();
```

```compile_fail
use anomalous_desktop_service::desktop_surface;
fn require<T: core::fmt::Debug>() {}
require::<desktop_surface>();
```

```compile_fail
use anomalous_desktop_service::desktop_surface;
fn require<T: core::fmt::Display>() {}
require::<desktop_surface>();
```

```compile_fail
use anomalous_desktop_service::desktop_surface;
fn require<T: core::hash::Hash>() {}
require::<desktop_surface>();
```

```compile_fail
use anomalous_desktop_service::surface_retirement;
fn require<T: Clone>() {}
require::<surface_retirement>();
```

```compile_fail
use anomalous_desktop_service::surface_retirement;
fn require<T: core::fmt::Debug>() {}
require::<surface_retirement>();
```

```compile_fail
use anomalous_desktop_service::surface_retirement;
fn require<T: core::fmt::Display>() {}
require::<surface_retirement>();
```

```compile_fail
use anomalous_desktop_service::surface_retirement;
fn require<T: core::hash::Hash>() {}
require::<surface_retirement>();
```

```compile_fail
use anomalous_desktop_service::surface_construction_rejection;
fn require<T: Clone>() {}
require::<surface_construction_rejection>();
```

```compile_fail
use anomalous_desktop_service::submission_rejection;
fn require<T: Clone>() {}
require::<submission_rejection>();
```

```compile_fail
use anomalous_desktop_service::completion_rejection;
fn require<T: Clone>() {}
require::<completion_rejection>();
```

```compile_fail
use anomalous_desktop_service::presentation_rejection;
fn require<T: Clone>() {}
require::<presentation_rejection>();
```

```compile_fail
use anomalous_desktop_service::publication_rejection;
fn require<T: Clone>() {}
require::<publication_rejection>();
```

```compile_fail
use anomalous_desktop_service::surface_action_rejection;
fn require<T: Clone>() {}
require::<surface_action_rejection>();
```

```compile_fail
use anomalous_desktop_service::surface_focus_rejection;
fn require<T: Clone>() {}
require::<surface_focus_rejection>();
```

```compile_fail
use anomalous_desktop_service::surface_retirement_rejection;
fn require<T: Clone>() {}
require::<surface_retirement_rejection>();
```

### Borrowed views do not escape private storage

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: Clone>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: core::fmt::Debug>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: core::fmt::Display>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: core::hash::Hash>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: AsRef<[u8]>>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn require<T: Into<String>>() {}
require::<render_snapshot_view<'static>>();
```

```compile_fail
use anomalous_desktop_service::render_snapshot_view;
fn forge() -> render_snapshot_view<'static> {
    render_snapshot_view { snapshot: panic!() }
}
```

```compile_fail
use anomalous_desktop_service::{desktop_surface, render_lease, render_snapshot_view};
fn escape(surface: &desktop_surface, lease: &render_lease) -> render_snapshot_view<'static> {
    surface.render_candidate(lease).unwrap()
}
```

### Controls are not interchangeable

```compile_fail
use anomalous_desktop_scene::{scene_candidate, scene_revision};
use anomalous_desktop_service::{desktop_surface, presentation_control};
fn wrong(
    surface: &mut desktop_surface,
    control: &presentation_control,
    base: scene_revision,
    candidate: scene_candidate,
) {
    let _ = surface.submit_revision(control, base, candidate);
}
```

```compile_fail
use anomalous_desktop_service::{desktop_surface, prepared_presentation, surface_control};
fn wrong(
    surface: &mut desktop_surface,
    control: &surface_control,
    ready: prepared_presentation,
) {
    let _ = surface.attest_presented(control, ready);
}
```

Snapshot commands and fixed label selectors are intentionally copyable
descriptive data. They contain no private service brand, protected source,
action handle, focus handle, projection owner, or publication authority.
