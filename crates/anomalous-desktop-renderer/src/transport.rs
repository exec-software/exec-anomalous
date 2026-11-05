use anomalous_desktop_transport::{
    surface_snapshot_v1, transport_color, transport_command_kind, transport_depth,
    transport_disclosure, transport_focus, transport_label_selector, transport_motion,
    transport_render_command, transport_shape, transport_slot,
};

use crate::capsule::{label_selector, validate_fixture_projection_command};
use crate::raster::{begin_row, paint_command, paint_command_on_row};
use crate::{
    capsule_color, capsule_command_kind, capsule_rect, capsule_shape, capsule_slot,
    fixture_command_count, projection_command, renderer_error,
};

/// A validated transport snapshot and its bounded raster plan.
///
/// The snapshot remains borrowed for the lifetime of the plan so callers can
/// keep the copied semantic body alive while a shared syscall mailbox is
/// reused for display updates.
pub struct transport_render_snapshot<'a> {
    snapshot: &'a surface_snapshot_v1,
    commands: [transport_render_command; fixture_command_count],
    labels: [Option<label_selector>; fixture_command_count],
}

impl<'a> transport_render_snapshot<'a> {
    pub fn try_new(snapshot: &'a surface_snapshot_v1) -> Result<Self, renderer_error> {
        validate_state(snapshot)?;
        if snapshot.command_count() != fixture_command_count {
            return Err(renderer_error::wrong_command_count);
        }

        let viewport = snapshot.viewport();
        if viewport.x() != 0
            || viewport.y() != 0
            || u32::from(viewport.width()) != crate::frame_width
            || u32::from(viewport.height()) != crate::frame_height
        {
            return Err(renderer_error::wrong_viewport);
        }

        let first = snapshot
            .command(0)
            .ok_or(renderer_error::wrong_command_count)?;
        let mut commands = [first; fixture_command_count];
        let mut labels = [None; fixture_command_count];
        let mut index = 0;
        while index < fixture_command_count {
            let command = snapshot
                .command(index)
                .ok_or(renderer_error::wrong_command_count)?;
            let (projection, label) = projection_from_command(snapshot, command)?;
            validate_fixture_projection_command(index, &projection)?;
            commands[index] = command;
            labels[index] = label;
            index += 1;
        }
        Ok(Self {
            snapshot,
            commands,
            labels,
        })
    }

    #[must_use]
    pub const fn snapshot(&self) -> &surface_snapshot_v1 {
        self.snapshot
    }

    pub fn render_row(&self, row: u32, output: &mut [u8]) -> Result<(), renderer_error> {
        begin_row(row, output)?;
        let mut index = 0;
        while index < fixture_command_count {
            let command = self.commands[index];
            let bounds = command.bounds();
            paint_command_on_row(
                output,
                row,
                paint_command {
                    kind: map_kind(command.kind()),
                    shape: map_shape(command.shape()),
                    color: map_color(command.color()),
                    secondary_color: command.secondary_color().map(map_color),
                    slot: map_slot(command.slot()),
                    label: self.labels[index],
                    bounds: capsule_rect::try_new(
                        u32::from(bounds.x()),
                        u32::from(bounds.y()),
                        u32::from(bounds.width()),
                        u32::from(bounds.height()),
                    )?,
                    state_active: command.state_active(),
                },
            );
            index += 1;
        }
        Ok(())
    }
}

fn validate_state(snapshot: &surface_snapshot_v1) -> Result<(), renderer_error> {
    let state = snapshot.state();
    if state.primary_enabled()
        && !state.secondary_enabled()
        && state.focus() == transport_focus::primary
        && state.disclosure() == transport_disclosure::protected
        && state.depth() == transport_depth::standard
        && state.motion() == transport_motion::reduced
    {
        Ok(())
    } else {
        Err(renderer_error::wrong_state_ordinal)
    }
}

fn projection_from_command(
    snapshot: &surface_snapshot_v1,
    command: transport_render_command,
) -> Result<(projection_command<'static>, Option<label_selector>), renderer_error> {
    let bounds = command.bounds();
    let (label, text) = command_label(snapshot, command)?;
    Ok((
        projection_command::new(
            map_kind(command.kind()),
            map_shape(command.shape()),
            map_color(command.color()),
            command.secondary_color().map(map_color),
            map_slot(command.slot()),
            capsule_rect::try_new(
                u32::from(bounds.x()),
                u32::from(bounds.y()),
                u32::from(bounds.width()),
                u32::from(bounds.height()),
            )?,
            command.state_active(),
            text,
        ),
        label,
    ))
}

fn command_label(
    snapshot: &surface_snapshot_v1,
    command: transport_render_command,
) -> Result<(Option<label_selector>, Option<&'static str>), renderer_error> {
    if command.kind() != transport_command_kind::label {
        return Ok((None, None));
    }
    let index = match command.slot() {
        transport_slot::masthead => 1,
        transport_slot::navigation => 2,
        transport_slot::primary_action => 3,
        transport_slot::secondary_action => 4,
        transport_slot::heading => 6,
        transport_slot::status => 7,
        _ => return Err(renderer_error::capsule_mismatch),
    };
    match snapshot.label(index) {
        Some(transport_label_selector::anomalous) => {
            Ok((Some(label_selector::masthead), Some("Anomalous")))
        }
        Some(transport_label_selector::navigation) => {
            Ok((Some(label_selector::navigation), Some("Navigation")))
        }
        Some(transport_label_selector::open_shell) => {
            Ok((Some(label_selector::primary_action), Some("Open shell")))
        }
        Some(transport_label_selector::inspect_system) => Ok((
            Some(label_selector::secondary_action),
            Some("Inspect system"),
        )),
        Some(transport_label_selector::system_workspace) => {
            Ok((Some(label_selector::heading), Some("System workspace")))
        }
        Some(transport_label_selector::protected_status) => Ok((
            Some(label_selector::protected_status),
            Some("Protected status"),
        )),
        _ => Err(renderer_error::wrong_label),
    }
}

const fn map_kind(value: transport_command_kind) -> capsule_command_kind {
    match value {
        transport_command_kind::fill => capsule_command_kind::fill,
        transport_command_kind::outline => capsule_command_kind::outline,
        transport_command_kind::label => capsule_command_kind::label,
        transport_command_kind::enabled_marker => capsule_command_kind::enabled_marker,
        transport_command_kind::focus_outline => capsule_command_kind::focus_outline,
        transport_command_kind::shadow => capsule_command_kind::shadow,
        transport_command_kind::highlight => capsule_command_kind::highlight,
    }
}

const fn map_shape(value: transport_shape) -> capsule_shape {
    match value {
        transport_shape::rectangle => capsule_shape::rectangle,
        transport_shape::text => capsule_shape::text,
        transport_shape::enabled => capsule_shape::enabled,
        transport_shape::disabled => capsule_shape::disabled,
        transport_shape::focus_absent => capsule_shape::focus_absent,
        transport_shape::focus_double_outline => capsule_shape::focus_double_outline,
    }
}

const fn map_color(value: transport_color) -> capsule_color {
    match value {
        transport_color::canvas => capsule_color::canvas,
        transport_color::surface => capsule_color::surface,
        transport_color::primary_fill => capsule_color::primary_fill,
        transport_color::secondary_fill => capsule_color::secondary_fill,
        transport_color::disabled => capsule_color::disabled,
        transport_color::primary_text => capsule_color::primary_text,
        transport_color::secondary_text => capsule_color::secondary_text,
        transport_color::on_accent => capsule_color::on_accent,
        transport_color::outline => capsule_color::outline,
        transport_color::focus_outer => capsule_color::focus_outer,
        transport_color::focus_inner => capsule_color::focus_inner,
        transport_color::public_status => capsule_color::public_status,
        transport_color::protected_status => capsule_color::protected_status,
        transport_color::shadow => capsule_color::shadow,
        transport_color::highlight => capsule_color::highlight,
    }
}

const fn map_slot(value: transport_slot) -> capsule_slot {
    match value {
        transport_slot::root => capsule_slot::root,
        transport_slot::masthead => capsule_slot::masthead,
        transport_slot::navigation => capsule_slot::navigation,
        transport_slot::primary_action => capsule_slot::primary_action,
        transport_slot::secondary_action => capsule_slot::secondary_action,
        transport_slot::content => capsule_slot::content,
        transport_slot::heading => capsule_slot::heading,
        transport_slot::status => capsule_slot::status,
    }
}

#[cfg(test)]
mod tests {
    use anomalous_desktop_transport::{
        surface_snapshot_binding, transport_identity, transport_rect, transport_scene_state,
    };

    use super::*;
    use crate::{frame_height, render_capsule, render_row, row_byte_count};

    fn exact_snapshot() -> surface_snapshot_v1 {
        encode_snapshot(exact_state(), commands_from_capsule())
    }

    fn exact_state() -> transport_scene_state {
        transport_scene_state::try_new(
            true,
            false,
            transport_focus::primary,
            transport_disclosure::protected,
            transport_depth::standard,
            transport_motion::reduced,
        )
        .unwrap()
    }

    fn encode_snapshot(
        state: transport_scene_state,
        commands: [transport_render_command; fixture_command_count],
    ) -> surface_snapshot_v1 {
        let binding = surface_snapshot_binding::try_new(
            transport_identity::try_new(51, 3).unwrap(),
            transport_identity::try_new(41, 7).unwrap(),
            1,
            2,
            transport_identity::try_new(61, 1).unwrap(),
            1,
        )
        .unwrap();
        let labels = [
            transport_label_selector::anomalous_desktop,
            transport_label_selector::anomalous,
            transport_label_selector::navigation,
            transport_label_selector::open_shell,
            transport_label_selector::inspect_system,
            transport_label_selector::workspace,
            transport_label_selector::system_workspace,
            transport_label_selector::protected_status,
        ];
        surface_snapshot_v1::try_encode(
            binding,
            state,
            transport_rect::try_new(0, 0, 1024, 768).unwrap(),
            labels,
            &commands,
        )
        .unwrap()
    }

    fn commands_from_capsule() -> [transport_render_command; fixture_command_count] {
        let capsule = render_capsule::fixture();
        core::array::from_fn(|index| {
            let command = capsule.command(index);
            transport_render_command::new(
                transport_command_kind::from_raw(command.kind as u8 + 1).unwrap(),
                transport_shape::from_raw(command.shape as u8 + 1).unwrap(),
                transport_color::from_raw(command.color as u8 + 1).unwrap(),
                command
                    .secondary_color
                    .map(|color| transport_color::from_raw(color as u8 + 1).unwrap()),
                transport_slot::from_raw(command.slot as u8 + 1).unwrap(),
                command.state_active,
                transport_rect::try_new(
                    command.bounds.x() as u16,
                    command.bounds.y() as u16,
                    command.bounds.width() as u16,
                    command.bounds.height() as u16,
                )
                .unwrap(),
            )
        })
    }

    #[test]
    fn exact_transport_snapshot_matches_capsule_for_every_row() {
        let snapshot = exact_snapshot();
        let retained_bytes = *snapshot.as_bytes();
        let renderer = transport_render_snapshot::try_new(&snapshot).unwrap();
        let capsule = render_capsule::fixture();
        let mut actual = [0; row_byte_count];
        let mut expected = [0; row_byte_count];
        for row in 0..frame_height {
            renderer.render_row(row, &mut actual).unwrap();
            render_row(&capsule, row, &mut expected).unwrap();
            assert_eq!(actual, expected);
        }
        assert_eq!(renderer.snapshot().as_bytes(), &retained_bytes);
    }

    #[test]
    fn rejects_non_ordinal_61_state() {
        let state = transport_scene_state::try_new(
            true,
            false,
            transport_focus::primary,
            transport_disclosure::protected,
            transport_depth::standard,
            transport_motion::standard,
        )
        .unwrap();
        let snapshot = encode_snapshot(state, commands_from_capsule());
        assert_eq!(
            transport_render_snapshot::try_new(&snapshot).err(),
            Some(renderer_error::wrong_state_ordinal)
        );
    }

    #[test]
    fn rejects_noncanonical_transport_command() {
        let mut commands = commands_from_capsule();
        let first = commands[0];
        commands[0] = transport_render_command::new(
            first.kind(),
            first.shape(),
            first.color(),
            first.secondary_color(),
            first.slot(),
            false,
            first.bounds(),
        );
        let snapshot = encode_snapshot(exact_state(), commands);
        assert_eq!(
            transport_render_snapshot::try_new(&snapshot).err(),
            Some(renderer_error::capsule_mismatch)
        );
    }

    #[test]
    fn row_contract_remains_enforced() {
        let snapshot = exact_snapshot();
        let renderer = transport_render_snapshot::try_new(&snapshot).unwrap();
        let mut row = [0; row_byte_count];
        assert_eq!(
            renderer.render_row(frame_height, &mut row),
            Err(renderer_error::wrong_row)
        );
        assert_eq!(
            renderer.render_row(0, &mut row[..row_byte_count - 1]),
            Err(renderer_error::wrong_row_buffer_length)
        );
    }
}
