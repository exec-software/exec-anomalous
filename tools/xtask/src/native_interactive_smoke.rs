//! Physical input and durable saves through independent native owners.
use super::*;
pub(super) const feature: &str = "native_interactive_editor_test";
pub(super) fn selected(features: &[&str]) -> bool {
    features.contains(&feature)
        && native_durable_smoke::selected(
            &features
                .iter()
                .copied()
                .filter(|v| {
                    !matches!(
                        *v,
                        "native_interactive_editor_test" | "native_repeated_editor_test"
                    )
                })
                .chain(core::iter::once(native_durable_smoke::feature))
                .collect::<Vec<_>>(),
        )
}
pub(super) fn build() -> Result<built_image, String> {
    let image = build_image_with_context(&[native_durable_smoke::feature, feature], None)?;
    println!(
        "Interactive native editor image inspected: {}",
        image.path.display()
    );
    Ok(image)
}
pub(super) fn smoke() -> Result<(), String> {
    let image = build()?;
    let root = repository_root()?;
    run_checked(
        &root,
        "python3",
        [
            "scripts/native-interactive-proof.py".into(),
            "--image".into(),
            image.path.into_os_string(),
        ],
    )
}

/// Render literal host observations, never run/replay the guest editor model.
pub(super) fn render_observation() -> Result<(), String> {
    use anomalous_editor_contract::{
        interactive_audit, interactive_frame_byte_count, render_interactive_audit,
    };
    use std::io::{Read, Write};
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(129)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let audit = interactive_audit::decode(&bytes).map_err(|e| format!("invalid ANUI: {e:?}"))?;
    let mut pixels = [0; interactive_frame_byte_count];
    render_interactive_audit(&audit, &mut pixels).map_err(|e| format!("invalid frame: {e:?}"))?;
    std::io::stdout()
        .write_all(&pixels)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interactive_selection_is_exclusive_and_lower_than_existing_replacement_gates() {
        assert!(selected(&[feature]));
        assert!(selected(&[feature, native_durable_smoke::feature]));
        assert!(!native_durable_smoke::selected(&[
            feature,
            native_durable_smoke::feature
        ]));
        for (lower, select) in [
            (
                native_durable_smoke::feature,
                native_durable_smoke::selected as fn(&[&str]) -> bool,
            ),
            (
                native_repeated_smoke::feature,
                native_repeated_smoke::selected,
            ),
            (
                native_document_smoke::feature,
                native_document_smoke::selected,
            ),
            (
                native_runtime_smoke::feature,
                native_runtime_smoke::selected,
            ),
            (
                native_application_smoke::feature,
                native_application_smoke::selected,
            ),
            (workspace_dev::feature, workspace_dev::selected),
            (owned_workspace::feature, owned_workspace::selected),
        ] {
            assert!(selected(&[feature, lower]), "{lower}");
            assert!(!select(&[feature, lower]), "{lower}");
        }
        for higher in [
            "native_durable_failure_test",
            "native_relaunch_test",
            "elf_relaunch_test",
            "elf_document_test",
            "elf_application_test",
            "linux_application_test",
            "veil_application_test",
            "shared_veil_test",
            "linux_tls_test",
            "linux_loader_entry_test",
        ] {
            assert!(!selected(&[feature, higher]), "{higher}");
        }
    }
}
