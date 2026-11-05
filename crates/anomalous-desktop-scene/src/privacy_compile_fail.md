## Compile-fail ownership and disclosure matrix

The scene's text, canonical owners, rejection owners, local handles, and
borrowed projection views stay non-cloning and non-formatting. Each trait block
is independent so adding one accidental surface makes that block compile and
fails the documentation test. `Copy` implies `Clone`, so every isolated
`Clone` check also guards against an accidental `Copy` implementation.

### `declared_public_text`

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: Clone>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: core::fmt::Debug>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: core::fmt::Display>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: core::hash::Hash>() {}
require::<declared_public_text>();
```

### `protected_text`

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: Clone>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: core::fmt::Debug>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: core::fmt::Display>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: core::hash::Hash>() {}
require::<protected_text>();
```

### `scene_label`

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: Clone>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: core::fmt::Debug>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: core::fmt::Display>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: core::hash::Hash>() {}
require::<scene_label>();
```

### `scene_node_definition`

```compile_fail
use anomalous_desktop_scene::scene_node_definition;
fn require<T: Clone>() {}
require::<scene_node_definition>();
```

```compile_fail
use anomalous_desktop_scene::scene_node_definition;
fn require<T: core::fmt::Debug>() {}
require::<scene_node_definition>();
```

```compile_fail
use anomalous_desktop_scene::scene_node_definition;
fn require<T: core::fmt::Display>() {}
require::<scene_node_definition>();
```

```compile_fail
use anomalous_desktop_scene::scene_node_definition;
fn require<T: core::hash::Hash>() {}
require::<scene_node_definition>();
```

### `scene_candidate`

```compile_fail
use anomalous_desktop_scene::scene_candidate;
fn require<T: Clone>() {}
require::<scene_candidate>();
```

```compile_fail
use anomalous_desktop_scene::scene_candidate;
fn require<T: core::fmt::Debug>() {}
require::<scene_candidate>();
```

```compile_fail
use anomalous_desktop_scene::scene_candidate;
fn require<T: core::fmt::Display>() {}
require::<scene_candidate>();
```

```compile_fail
use anomalous_desktop_scene::scene_candidate;
fn require<T: core::hash::Hash>() {}
require::<scene_candidate>();
```

### `scene_control`

```compile_fail
use anomalous_desktop_scene::scene_control;
fn require<T: Clone>() {}
require::<scene_control>();
```

```compile_fail
use anomalous_desktop_scene::scene_control;
fn require<T: core::fmt::Debug>() {}
require::<scene_control>();
```

```compile_fail
use anomalous_desktop_scene::scene_control;
fn require<T: core::fmt::Display>() {}
require::<scene_control>();
```

```compile_fail
use anomalous_desktop_scene::scene_control;
fn require<T: core::hash::Hash>() {}
require::<scene_control>();
```

### `scene_construction_rejection`

```compile_fail
use anomalous_desktop_scene::scene_construction_rejection;
fn require<T: Clone>() {}
require::<scene_construction_rejection>();
```

```compile_fail
use anomalous_desktop_scene::scene_construction_rejection;
fn require<T: core::fmt::Debug>() {}
require::<scene_construction_rejection>();
```

```compile_fail
use anomalous_desktop_scene::scene_construction_rejection;
fn require<T: core::fmt::Display>() {}
require::<scene_construction_rejection>();
```

```compile_fail
use anomalous_desktop_scene::scene_construction_rejection;
fn require<T: core::hash::Hash>() {}
require::<scene_construction_rejection>();
```

### `prepared_revision`

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn require<T: Clone>() {}
require::<prepared_revision>();
```

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn require<T: core::fmt::Debug>() {}
require::<prepared_revision>();
```

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn require<T: core::fmt::Display>() {}
require::<prepared_revision>();
```

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn require<T: core::hash::Hash>() {}
require::<prepared_revision>();
```

### `prepared_render_preview`

```compile_fail
use anomalous_desktop_scene::prepared_render_preview;
fn require<T: Clone>() {}
require::<prepared_render_preview<'static>>();
```

```compile_fail
use anomalous_desktop_scene::prepared_render_preview;
fn require<T: core::fmt::Debug>() {}
require::<prepared_render_preview<'static>>();
```

```compile_fail
use anomalous_desktop_scene::prepared_render_preview;
fn require<T: core::fmt::Display>() {}
require::<prepared_render_preview<'static>>();
```

```compile_fail
use anomalous_desktop_scene::prepared_render_preview;
fn require<T: core::hash::Hash>() {}
require::<prepared_render_preview<'static>>();
```

### `prepare_rejection`

```compile_fail
use anomalous_desktop_scene::prepare_rejection;
fn require<T: Clone>() {}
require::<prepare_rejection>();
```

```compile_fail
use anomalous_desktop_scene::prepare_rejection;
fn require<T: core::fmt::Debug>() {}
require::<prepare_rejection>();
```

```compile_fail
use anomalous_desktop_scene::prepare_rejection;
fn require<T: core::fmt::Display>() {}
require::<prepare_rejection>();
```

```compile_fail
use anomalous_desktop_scene::prepare_rejection;
fn require<T: core::hash::Hash>() {}
require::<prepare_rejection>();
```

### `commit_rejection`

```compile_fail
use anomalous_desktop_scene::commit_rejection;
fn require<T: Clone>() {}
require::<commit_rejection>();
```

```compile_fail
use anomalous_desktop_scene::commit_rejection;
fn require<T: core::fmt::Debug>() {}
require::<commit_rejection>();
```

```compile_fail
use anomalous_desktop_scene::commit_rejection;
fn require<T: core::fmt::Display>() {}
require::<commit_rejection>();
```

```compile_fail
use anomalous_desktop_scene::commit_rejection;
fn require<T: core::hash::Hash>() {}
require::<commit_rejection>();
```

### `desktop_scene`

```compile_fail
use anomalous_desktop_scene::desktop_scene;
fn require<T: Clone>() {}
require::<desktop_scene>();
```

```compile_fail
use anomalous_desktop_scene::desktop_scene;
fn require<T: core::fmt::Debug>() {}
require::<desktop_scene>();
```

```compile_fail
use anomalous_desktop_scene::desktop_scene;
fn require<T: core::fmt::Display>() {}
require::<desktop_scene>();
```

```compile_fail
use anomalous_desktop_scene::desktop_scene;
fn require<T: core::hash::Hash>() {}
require::<desktop_scene>();
```

### `local_action_handle`

```compile_fail
use anomalous_desktop_scene::local_action_handle;
fn require<T: Clone>() {}
require::<local_action_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_action_handle;
fn require<T: core::fmt::Debug>() {}
require::<local_action_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_action_handle;
fn require<T: core::fmt::Display>() {}
require::<local_action_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_action_handle;
fn require<T: core::hash::Hash>() {}
require::<local_action_handle>();
```

### `action_resolution`

```compile_fail
use anomalous_desktop_scene::action_resolution;
fn require<T: Clone>() {}
require::<action_resolution>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution;
fn require<T: core::fmt::Debug>() {}
require::<action_resolution>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution;
fn require<T: core::fmt::Display>() {}
require::<action_resolution>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution;
fn require<T: core::hash::Hash>() {}
require::<action_resolution>();
```

### `action_resolution_rejection`

```compile_fail
use anomalous_desktop_scene::action_resolution_rejection;
fn require<T: Clone>() {}
require::<action_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution_rejection;
fn require<T: core::fmt::Debug>() {}
require::<action_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution_rejection;
fn require<T: core::fmt::Display>() {}
require::<action_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::action_resolution_rejection;
fn require<T: core::hash::Hash>() {}
require::<action_resolution_rejection>();
```

### `local_focus_handle`

```compile_fail
use anomalous_desktop_scene::local_focus_handle;
fn require<T: Clone>() {}
require::<local_focus_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_focus_handle;
fn require<T: core::fmt::Debug>() {}
require::<local_focus_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_focus_handle;
fn require<T: core::fmt::Display>() {}
require::<local_focus_handle>();
```

```compile_fail
use anomalous_desktop_scene::local_focus_handle;
fn require<T: core::hash::Hash>() {}
require::<local_focus_handle>();
```

### `focus_resolution`

```compile_fail
use anomalous_desktop_scene::focus_resolution;
fn require<T: Clone>() {}
require::<focus_resolution>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution;
fn require<T: core::fmt::Debug>() {}
require::<focus_resolution>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution;
fn require<T: core::fmt::Display>() {}
require::<focus_resolution>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution;
fn require<T: core::hash::Hash>() {}
require::<focus_resolution>();
```

### `focus_resolution_rejection`

```compile_fail
use anomalous_desktop_scene::focus_resolution_rejection;
fn require<T: Clone>() {}
require::<focus_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution_rejection;
fn require<T: core::fmt::Debug>() {}
require::<focus_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution_rejection;
fn require<T: core::fmt::Display>() {}
require::<focus_resolution_rejection>();
```

```compile_fail
use anomalous_desktop_scene::focus_resolution_rejection;
fn require<T: core::hash::Hash>() {}
require::<focus_resolution_rejection>();
```

### `scene_retirement`

```compile_fail
use anomalous_desktop_scene::scene_retirement;
fn require<T: Clone>() {}
require::<scene_retirement>();
```

```compile_fail
use anomalous_desktop_scene::scene_retirement;
fn require<T: core::fmt::Debug>() {}
require::<scene_retirement>();
```

```compile_fail
use anomalous_desktop_scene::scene_retirement;
fn require<T: core::fmt::Display>() {}
require::<scene_retirement>();
```

```compile_fail
use anomalous_desktop_scene::scene_retirement;
fn require<T: core::hash::Hash>() {}
require::<scene_retirement>();
```

### `retirement_rejection`

```compile_fail
use anomalous_desktop_scene::retirement_rejection;
fn require<T: Clone>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_scene::retirement_rejection;
fn require<T: core::fmt::Debug>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_scene::retirement_rejection;
fn require<T: core::fmt::Display>() {}
require::<retirement_rejection>();
```

```compile_fail
use anomalous_desktop_scene::retirement_rejection;
fn require<T: core::hash::Hash>() {}
require::<retirement_rejection>();
```

### `scene_projections`

```compile_fail
use anomalous_desktop_scene::scene_projections;
fn require<T: Clone>() {}
require::<scene_projections<'static>>();
```

```compile_fail
use anomalous_desktop_scene::scene_projections;
fn require<T: core::fmt::Debug>() {}
require::<scene_projections<'static>>();
```

```compile_fail
use anomalous_desktop_scene::scene_projections;
fn require<T: core::fmt::Display>() {}
require::<scene_projections<'static>>();
```

```compile_fail
use anomalous_desktop_scene::scene_projections;
fn require<T: core::hash::Hash>() {}
require::<scene_projections<'static>>();
```

### `accessible_render_plan`

```compile_fail
use anomalous_desktop_scene::accessible_render_plan;
fn require<T: Clone>() {}
require::<accessible_render_plan<'static>>();
```

```compile_fail
use anomalous_desktop_scene::accessible_render_plan;
fn require<T: core::fmt::Debug>() {}
require::<accessible_render_plan<'static>>();
```

```compile_fail
use anomalous_desktop_scene::accessible_render_plan;
fn require<T: core::fmt::Display>() {}
require::<accessible_render_plan<'static>>();
```

```compile_fail
use anomalous_desktop_scene::accessible_render_plan;
fn require<T: core::hash::Hash>() {}
require::<accessible_render_plan<'static>>();
```

### `machine_scene_projection`

```compile_fail
use anomalous_desktop_scene::machine_scene_projection;
fn require<T: Clone>() {}
require::<machine_scene_projection<'static>>();
```

```compile_fail
use anomalous_desktop_scene::machine_scene_projection;
fn require<T: core::fmt::Debug>() {}
require::<machine_scene_projection<'static>>();
```

```compile_fail
use anomalous_desktop_scene::machine_scene_projection;
fn require<T: core::fmt::Display>() {}
require::<machine_scene_projection<'static>>();
```

```compile_fail
use anomalous_desktop_scene::machine_scene_projection;
fn require<T: core::hash::Hash>() {}
require::<machine_scene_projection<'static>>();
```

### `projection_label`

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: Clone>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: core::fmt::Debug>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: core::fmt::Display>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: core::hash::Hash>() {}
require::<projection_label<'static>>();
```

## String-like convenience conversions stay absent

Text owners and borrowed projection labels expose text only through explicitly
named disclosure methods. None becomes string-like through a blanket trait.

### `declared_public_text`

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: core::ops::Deref<Target = str>>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: AsRef<str>>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: AsRef<[u8]>>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: Into<std::string::String>>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: Into<std::vec::Vec<u8>>>() {}
require::<declared_public_text>();
```

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn require<T: for<'a> From<&'a str>>() {}
require::<declared_public_text>();
```

### `protected_text`

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: core::ops::Deref<Target = str>>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: AsRef<str>>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: AsRef<[u8]>>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: Into<std::string::String>>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: Into<std::vec::Vec<u8>>>() {}
require::<protected_text>();
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn require<T: for<'a> From<&'a str>>() {}
require::<protected_text>();
```

### `scene_label`

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: core::ops::Deref<Target = str>>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: AsRef<str>>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: AsRef<[u8]>>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: Into<std::string::String>>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: Into<std::vec::Vec<u8>>>() {}
require::<scene_label>();
```

```compile_fail
use anomalous_desktop_scene::scene_label;
fn require<T: for<'a> From<&'a str>>() {}
require::<scene_label>();
```

### `projection_label`

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: core::ops::Deref<Target = str>>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: AsRef<str>>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: AsRef<[u8]>>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: Into<std::string::String>>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: Into<std::vec::Vec<u8>>>() {}
require::<projection_label<'static>>();
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn require<T: From<&'static str>>() {}
require::<projection_label<'static>>();
```

## Private fields prevent branded-owner and handle fabrication

Public scalar identities and references are deliberately copyable correlation
data. The owners that add the private scene instance, unpublished revision, or
borrowed committed owner cannot be rebuilt with public fields.

```compile_fail
use anomalous_desktop_scene::declared_public_text;
fn rebuild(value: declared_public_text) -> declared_public_text {
    declared_public_text(value.0)
}
```

```compile_fail
use anomalous_desktop_scene::protected_text;
fn rebuild(value: protected_text) -> protected_text {
    protected_text(value.0)
}
```

```compile_fail
use anomalous_desktop_scene::scene_control;
fn rebuild(value: scene_control) -> scene_control {
    scene_control { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn rebuild(value: prepared_revision) -> prepared_revision {
    prepared_revision { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::prepared_render_preview;
fn rebuild<'a>(value: prepared_render_preview<'a>) -> prepared_render_preview<'a> {
    prepared_render_preview { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::desktop_scene;
fn rebuild(value: desktop_scene) -> desktop_scene {
    desktop_scene { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::local_action_handle;
fn rebuild(value: local_action_handle) -> local_action_handle {
    local_action_handle { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::local_focus_handle;
fn rebuild(value: local_focus_handle) -> local_focus_handle {
    local_focus_handle { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::scene_action_reference;
fn rebuild(value: scene_action_reference) -> scene_action_reference {
    scene_action_reference { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::scene_focus_reference;
fn rebuild(value: scene_focus_reference) -> scene_focus_reference {
    scene_focus_reference { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::scene_projections;
fn rebuild<'a>(value: scene_projections<'a>) -> scene_projections<'a> {
    scene_projections { ..value }
}
```

```compile_fail
use anomalous_desktop_scene::projection_label;
fn rebuild<'a>(value: projection_label<'a>) -> projection_label<'a> {
    projection_label { ..value }
}
```

## Prepared render previews stay descriptive and borrowed

Prepared paint commands deliberately omit the revision-bound action reference.
The preview also cannot be used after consuming its prepared owner to recover
the source candidate.

```compile_fail
use anomalous_desktop_scene::prepared_paint_command;
fn extract_action(command: prepared_paint_command) {
    let _ = command.action();
}
```

```compile_fail
use anomalous_desktop_scene::prepared_revision;
fn recover_while_borrowed(prepared: prepared_revision) {
    let preview = prepared.preview();
    let _candidate = prepared.into_candidate();
    let _ = preview.command_count();
}
```

## Public references never substitute for local branded handles

An action reference is copyable correlation data. Resolution accepts only the
private, instance-bound handle minted by a committed projection.

```compile_fail
use anomalous_desktop_scene::{desktop_scene, scene_action_reference};
fn resolve(scene: &desktop_scene, reference: scene_action_reference) {
    let _ = scene.resolve_action(reference);
}
```

A returned focus reference is likewise correlation data, not a traversal
handle.

```compile_fail
use anomalous_desktop_scene::{desktop_scene, focus_direction, scene_focus_reference};
fn traverse(scene: &desktop_scene, reference: scene_focus_reference) {
    let _ = scene.traverse_focus(reference, focus_direction::forward);
}
```
