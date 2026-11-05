
Local authority cannot be reconstructed from descriptive values:

```compile_fail
use anomalous_desktop_workspace::desktop_control;
let control = desktop_control { instance: 1 };
```

```compile_fail
use anomalous_desktop_workspace::client_endpoint;
let client = client_endpoint { instance: 1, slot: 0, generation: 1 };
```

```compile_fail
use anomalous_desktop_workspace::update_ticket;
let ticket = update_ticket { instance: 1, transaction: 1 };
```

Tickets and controls cannot be cloned or implicitly formatted:

```compile_fail
use anomalous_desktop_workspace::update_ticket;
fn duplicate(ticket: &update_ticket) -> update_ticket { (*ticket).clone() }
```

```compile_fail
use anomalous_desktop_workspace::desktop_control;
fn duplicate(control: &desktop_control) -> desktop_control { (*control).clone() }
```

```compile_fail
use anomalous_desktop_workspace::client_endpoint;
fn disclose(client: &client_endpoint) { let _ = format!("{client:?}"); }
```

Publication consumes its exact ticket:

```compile_fail
use anomalous_desktop_workspace::desktop;
let (mut model, control, ticket) = desktop::new_demo(1024, 768).unwrap();
model.begin(&control, &ticket).unwrap();
model.attest_presented(&control, &ticket).unwrap();
let _ = model.publish(&control, ticket);
let _ = model.publish(&control, ticket);
```

A current borrowed semantic view cannot coexist with a mutable presentation
transition. Previously copied public facts remain historical observations.

```compile_fail
use anomalous_desktop_workspace::{desktop, client_origin, client_grants, desktop_command, theme_mode};
let (mut model, control, initial) = desktop::new_demo(1024, 768).unwrap();
let client = model.bind_client(&control, client_origin::human, client_grants::all).unwrap();
model.begin(&control, &initial).unwrap();
model.attest_presented(&control, &initial).unwrap();
let _ = model.publish(&control, initial);
let ticket = model.prepare(&client, 1, desktop_command::set_theme(theme_mode::dark)).unwrap();
let current = model.observe(&client).unwrap();
model.begin(&control, &ticket).unwrap();
let _ = current.revision();
```
Content producer authority cannot be reconstructed from public identities:

```compile_fail
use anomalous_desktop_workspace::{surface_owner, window_id};
fn forge(window: window_id) -> surface_owner {
    surface_owner { instance: 1, client_slot: 0, client_generation: 1, window }
}
```

Scoped content owners cannot be duplicated through cloning:

```compile_fail
use anomalous_desktop_workspace::surface_owner;
fn duplicate(owner: &surface_owner) -> surface_owner { (*owner).clone() }
```
