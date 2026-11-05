use anomalous_desktop_scene::{
    accessibility_node, declared_scene_state, desktop_scene, machine_action_descriptor,
    machine_node, paint_command, projection_label, scene_identity, scene_rect, scene_revision,
};

/// Read-only view of the one publicly presented semantic revision.
///
/// Unlike the scene crate's raw projection owners, this view cannot mint local
/// action or focus handles. The surface service remains the sole owner of the
/// canonical scene and exposes those capabilities only through its branded
/// wrapper operations.
pub struct presented_scene_view<'a> {
    scene: &'a desktop_scene,
}

impl<'a> presented_scene_view<'a> {
    pub(crate) const fn new(scene: &'a desktop_scene) -> Self {
        Self { scene }
    }

    #[must_use]
    pub const fn identity(&self) -> scene_identity {
        self.scene.identity()
    }

    #[must_use]
    pub const fn revision(&self) -> scene_revision {
        self.scene.revision()
    }

    #[must_use]
    pub const fn state(&self) -> declared_scene_state {
        self.scene.state()
    }

    #[must_use]
    pub fn viewport(&self) -> scene_rect {
        self.scene.projections().render_plan().viewport()
    }

    #[must_use]
    pub fn render_command_count(&self) -> usize {
        self.scene.projections().render_plan().command_count()
    }

    #[must_use]
    pub fn render_command(&self, index: usize) -> Option<paint_command> {
        self.scene.projections().render_plan().command(index)
    }

    #[must_use]
    pub fn accessibility_node_count(&self) -> usize {
        self.scene
            .projections()
            .render_plan()
            .accessibility_node_count()
    }

    #[must_use]
    pub fn accessibility_node(&self, index: usize) -> Option<accessibility_node> {
        self.scene
            .projections()
            .render_plan()
            .accessibility_node(index)
    }

    #[must_use]
    pub fn accessibility_label(&self, index: usize) -> Option<projection_label<'_>> {
        self.scene.projections().render_plan().label(index)
    }

    #[must_use]
    pub fn machine_node_count(&self) -> usize {
        self.scene.projections().machine_projection().node_count()
    }

    #[must_use]
    pub fn machine_node(&self, index: usize) -> Option<machine_node> {
        self.scene.projections().machine_projection().node(index)
    }

    #[must_use]
    pub fn machine_label(&self, index: usize) -> Option<projection_label<'_>> {
        self.scene.projections().machine_projection().label(index)
    }

    #[must_use]
    pub fn machine_action_count(&self) -> usize {
        self.scene.projections().machine_projection().action_count()
    }

    #[must_use]
    pub fn machine_action(&self, index: usize) -> Option<machine_action_descriptor> {
        self.scene.projections().machine_projection().action(index)
    }
}
