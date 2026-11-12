.PHONY: owned-workspace-cpu-image owned-workspace-cpu-inspect owned-workspace-cpu-smoke
.PHONY: owned-workspace-image owned-workspace-inspect owned-workspace-smoke
.PHONY: elf-relaunch-inspect elf-relaunch-image elf-relaunch-smoke native-relaunch-inspect native-relaunch-image native-relaunch-smoke elf-document-inspect elf-document-image elf-document-smoke native-runtime-inspect native-runtime-image native-runtime-smoke native-program-inspect native-program-image native-program-smoke native-document-inspect native-document-image native-document-smoke shared-veil-inspect shared-veil-image shared-veil-smoke veil-application-inspect veil-application-image veil-application-smoke linux-application-inspect linux-application-image linux-application-smoke elf-application-inspect elf-application-image elf-application-smoke native-application-inspect native-application-image native-application-smoke ny-object-inspect ny-object-image ny-object-smoke application-profile-image application-profile-smoke owned-dispatcher-image owned-dispatcher-smoke dispatcher-image dispatcher-smoke workspace-image workspace-run workspace-smoke workspace-editor-smoke workspace-virtualbox check circle-smoke clean control-smoke desktop-image desktop-run desktop-smoke desktop-virtualbox display-demo display-vessel-smoke fault-smoke image input-wait-smoke interrupt-smoke memory-smoke normal-exclusion rite-smoke run scheduler-smoke semantic-presentation-all-features-link semantic-presentation-inspect semantic-presentation-smoke semantic-surface-transport-all-features-link semantic-surface-transport-inspect semantic-surface-transport-smoke shell-smoke shell-vessel-smoke smoke supervised-renderer-all-features-link supervised-renderer-demo supervised-renderer-inspect supervised-renderer-smoke supervision-smoke virtualbox

image:
	cargo xtask image

run:
	cargo xtask qemu

smoke:
	cargo xtask smoke

fault-smoke:
	cargo xtask fault-smoke

circle-smoke:
	cargo xtask circle-smoke

control-smoke:
	cargo xtask control-smoke

rite-smoke:
	cargo xtask rite-smoke

dispatcher-image:
	cargo xtask dispatcher-image

dispatcher-smoke:
	cargo xtask dispatcher-smoke

.PHONY: linux-loader-entry-inspect linux-loader-entry-image linux-loader-entry-smoke
linux-loader-entry-inspect:
	cargo xtask linux-loader-entry-inspect

linux-loader-entry-image:
	cargo xtask build-linux-loader-entry

linux-loader-entry-smoke:
	cargo xtask smoke-linux-loader-entry

.PHONY: linux-tls-inspect linux-tls-image linux-tls-smoke
linux-tls-inspect:
	cargo xtask linux-tls-inspect

linux-tls-image:
	cargo xtask build-linux-tls

linux-tls-smoke:
	cargo xtask smoke-linux-tls

shared-veil-inspect:
	cargo xtask shared-veil-inspect

shared-veil-image:
	cargo xtask build-shared-veil

shared-veil-smoke:
	cargo xtask smoke-shared-veil

veil-application-inspect:
	cargo xtask veil-application-inspect

veil-application-image:
	cargo xtask build-veil-application

veil-application-smoke:
	cargo xtask smoke-veil-application

linux-application-inspect:
	cargo xtask linux-application-inspect

linux-application-image:
	cargo xtask build-linux-application

linux-application-smoke:
	cargo xtask smoke-linux-application

elf-application-inspect:
	cargo xtask elf-application-inspect

elf-application-image:
	cargo xtask elf-application-image

elf-application-smoke:
	cargo xtask elf-application-smoke

elf-relaunch-inspect:
	cargo xtask elf-relaunch-inspect

elf-relaunch-image:
	cargo xtask elf-relaunch-image

elf-relaunch-smoke:
	cargo xtask elf-relaunch-smoke

native-relaunch-inspect:
	cargo xtask native-relaunch-inspect

native-relaunch-image:
	cargo xtask native-relaunch-image

native-relaunch-smoke:
	cargo xtask native-relaunch-smoke

elf-document-inspect:
	cargo xtask elf-document-inspect

elf-document-image:
	cargo xtask elf-document-image

elf-document-smoke:
	cargo xtask elf-document-smoke

native-document-inspect:
	cargo xtask native-document-inspect

native-document-image:
	cargo xtask native-document-image

native-document-smoke:
	cargo xtask native-document-smoke

native-runtime-inspect:
	cargo xtask native-runtime-inspect

native-runtime-image:
	cargo xtask build-native-runtime

native-runtime-smoke:
	cargo xtask smoke-native-runtime

native-program-inspect:
	cargo xtask native-program-inspect

native-program-image:
	cargo xtask build-native-program

native-program-smoke:
	cargo xtask smoke-native-program

.PHONY: program-manifest-proof
program-manifest-proof:
	cargo xtask program-manifest-proof

native-application-inspect:
	cargo xtask native-application-inspect

native-application-image:
	cargo xtask native-application-image

native-application-smoke:
	cargo xtask native-application-smoke

ny-object-inspect:
	cargo xtask ny-object-inspect

ny-object-image:
	cargo xtask ny-object-image

ny-object-smoke:
	cargo xtask ny-object-smoke

application-profile-image:
	cargo xtask application-profile-image

application-profile-smoke:
	cargo xtask application-profile-smoke

owned-dispatcher-image:
	cargo xtask owned-dispatcher-image

owned-dispatcher-smoke:
	cargo xtask owned-dispatcher-smoke

.PHONY: owned-multi-region-image owned-multi-region-smoke
owned-multi-region-image:
	cargo xtask owned-multi-region-image

owned-multi-region-smoke:
	cargo xtask owned-multi-region-smoke

.PHONY: owned-source-image owned-source-smoke
owned-source-image:
	cargo xtask build-owned-source

owned-source-smoke:
	cargo xtask smoke-owned-source

.PHONY: owned-tls-image owned-tls-smoke
owned-tls-image:
	cargo xtask build-owned-tls

owned-tls-smoke:
	cargo xtask smoke-owned-tls

scheduler-smoke:
	cargo xtask scheduler-smoke

supervision-smoke:
	cargo xtask supervision-smoke

shell-vessel-smoke:
	cargo xtask shell-vessel-smoke

memory-smoke:
	cargo xtask memory-smoke

interrupt-smoke:
	cargo xtask interrupt-smoke

input-wait-smoke:
	cargo xtask input-wait-smoke

display-vessel-smoke:
	cargo xtask display-vessel-smoke

display-demo:
	cargo xtask display-demo

supervised-renderer-inspect:
	cargo xtask supervised-renderer-inspect

supervised-renderer-all-features-link:
	cargo xtask supervised-renderer-all-features-link

supervised-renderer-smoke:
	cargo xtask supervised-renderer-smoke

supervised-renderer-demo:
	cargo xtask supervised-renderer-demo

semantic-surface-transport-inspect:
	cargo xtask semantic-surface-transport-inspect

semantic-surface-transport-all-features-link:
	cargo xtask semantic-surface-transport-all-features-link

semantic-surface-transport-smoke:
	cargo xtask semantic-surface-transport-smoke

semantic-presentation-inspect:
	cargo xtask semantic-presentation-inspect

semantic-presentation-all-features-link:
	cargo xtask semantic-presentation-all-features-link

semantic-presentation-smoke:
	cargo xtask semantic-presentation-smoke

normal-exclusion:
	cargo xtask normal-exclusion

shell-smoke:
	cargo xtask shell-smoke

virtualbox: image
	./scripts/run-virtualbox.sh

desktop-image:
	cargo xtask desktop-image

desktop-run:
	cargo xtask desktop-run

desktop-smoke:
	cargo xtask desktop-smoke

desktop-virtualbox: desktop-image
	ANOMALOUS_VM_MODE=desktop ./scripts/run-virtualbox.sh

workspace-image:
	cargo xtask workspace-image

owned-workspace-cpu-image:
	cargo xtask owned-workspace-cpu-image

owned-workspace-cpu-inspect:
	cargo xtask owned-workspace-cpu-inspect

owned-workspace-cpu-smoke:
	cargo xtask owned-workspace-cpu-smoke

owned-workspace-image:
	cargo xtask owned-workspace-image

owned-workspace-inspect:
	cargo xtask owned-workspace-inspect

owned-workspace-smoke:
	cargo xtask owned-workspace-smoke

workspace-run:
	cargo xtask workspace-run

workspace-smoke:
	cargo xtask workspace-smoke

workspace-editor-smoke:
	cargo xtask workspace-editor-smoke

workspace-virtualbox: workspace-image
	ANOMALOUS_VM_MODE=workspace ./scripts/run-virtualbox.sh

.PHONY: oculus-smoke
oculus-smoke:
	$(MAKE) -C oculus smoke

.PHONY: archon-smoke archon-check
archon-smoke:
	$(MAKE) -C archon smoke

archon-check:
	$(MAKE) -C archon check

check:
	cargo fmt --all -- --check
	python3 scripts/native-interactive-proof-test.py
	python3 scripts/native-editor-workspace-proof-test.py
	python3 -B -m unittest discover -s tools -p test_verify_linux_process.py
	cargo test --package anomalous-abi
	cargo test --package anomalous-editor-contract
	cargo test --package anomalous-editor-contract --features remote_editor
	cargo test --package anomalous-native-runtime
	cargo test --package anomalous-program-manifest
	cargo test --package anomalous-document-protocol
	cargo test --package anomalous-document-protocol --features remote_workspace
	cargo test --package anomalous-elf-adapter
	cargo test --package anomalous-elf-adapter --features linux_image
	cargo test --package anomalous-veil-protocol
	cargo test --package anomalous-veil-protocol --features linux-policy
	cargo test --package anomalous-veil-protocol --features linux-tls-policy
	cargo test --package anomalous-veil-protocol --features linux-loader-policy
	cargo test --package anomalous-veil-protocol --features read-only-files
	cargo test --package anomalous-veil-protocol --features source-read
	cargo test --package anomalous-desktop-renderer
	cargo test --package anomalous-desktop-renderer --features scene_conformance
	cargo test --package anomalous-desktop-renderer --features transport
	cargo test --package anomalous-desktop-scene
	cargo test --package anomalous-desktop-service
	cargo test --package anomalous-desktop-recovery
	cargo test --package anomalous-desktop-theme
	cargo run --package anomalous-desktop-theme --example export_assets --target x86_64-unknown-linux-gnu -- --check
	cargo test --package anomalous-desktop-workspace
	cargo test --package anomalous-desktop-workspace --features native_editor_bridge
	cargo test --package anomalous-desktop-workspace --features native_editor_remote
	cargo test --package anomalous-window-surface
	cargo test --package anomalous-desktop-compositor
	cargo test --package anomalous-desktop-compositor --features native_editor_bridge
	cargo test --package anomalous-desktop-compositor --features native_editor_remote
	cargo check --package anomalous-desktop-workspace --package anomalous-desktop-compositor --features anomalous-desktop-workspace/native_editor_bridge
	cargo test --package anomalous-workspace-service --lib
	cargo test --package anomalous-workspace-service --lib --features native_remote
	mkdir -p target
	rustc --edition 2024 --test -D warnings vessels/anomalous-workspace-service/src/mailbox_copy.rs -o target/workspace-mailbox-copy-tests
	target/workspace-mailbox-copy-tests
	cargo test --package anomalous-desktop-transport
	cargo test --package anomalous-desktop-transport --features service_adapter
	cargo test --package anomalous-session
	cargo test --package anomalous-session-router
	cargo test --package kernel --lib
	cargo test --package kernel --lib --features circle_test
	cargo test --package kernel --lib --features control_test
	cargo test --package kernel --lib --features rite_test
	cargo test --package kernel --lib --features scheduler_test
	cargo test --package kernel --lib --features dispatcher_test
	cargo test --package kernel --lib --features native_application_test
	cargo test --package kernel --lib --features native_runtime_test
	cargo test --package kernel --lib --features native_durable_test
	cargo test --package kernel --lib --features native_durable_failure_test
	cargo test --package kernel --lib --features native_repeated_editor_test
	cargo test --package kernel --lib --features native_interactive_editor_test
	cargo test --package kernel --lib --features native_document_test
	cargo test --package kernel --lib --features elf_document_test
	cargo test --package kernel --lib --features native_relaunch_test
	cargo test --package kernel --lib --features elf_relaunch_test
	cargo test --package kernel --lib --features elf_application_test
	cargo test --package kernel --lib --features linux_application_test
	cargo test --package kernel --lib --features linux_tls_test
	cargo test --package kernel --lib --features linux_loader_entry_test
	cargo test --package kernel --lib --features linux_loader_entry_test,workspace_dev
	cargo test --package kernel --lib --features linux_loader_entry_test,linux_tls_test,shared_veil_test
	cargo test --package kernel --lib --features linux_loader_entry_test,application_profile_test
	cargo test --package kernel --lib --features linux_loader_entry_test,native_editor_workspace_test
	cargo test --package kernel --lib --features linux_tls_test,workspace_dev
	cargo test --package kernel --lib --features linux_tls_test,shared_veil_test
	cargo test --package kernel --lib --features linux_tls_test,application_profile_test
	cargo test --package kernel --lib --features linux_tls_test,native_editor_workspace_test
	cargo test --package kernel --lib --features shared_veil_test
	cargo test --package kernel --lib --features veil_application_test
	cargo test --package kernel --lib --features ny_object_test
	cargo test --package kernel --lib --features application_profile_test
	cargo test --package kernel --lib --features mixed_workspace_test
	cargo test --package kernel --lib --features native_editor_workspace_test
	cargo test --package kernel --lib --features owned_dispatcher_test
	cargo test --package kernel --lib --features owned_multi_region_test
	cargo test --package kernel --lib --features owned_source_test
	cargo test --package kernel --lib --features owned_source_test,workspace_dev
	cargo test --package kernel --lib --features owned_source_test,application_profile_test
	cargo test --package kernel --lib --features owned_tls_test
	cargo test --package kernel --lib --features owned_tls_test,workspace_dev
	cargo test --package kernel --lib --features owned_tls_test,application_profile_test
	cargo test --package kernel --lib --features owned_tls_test,owned_source_test
	cargo test --package kernel --lib --features supervision_test
	cargo test --package kernel --lib --features shell_vessel_test
	cargo test --package kernel --lib --features memory_test
	cargo test --package kernel --lib --features interrupt_test
	cargo test --package kernel --lib --features input_wait_test
	cargo test --package kernel --lib --features display_vessel_test
	cargo test --package kernel --lib --features supervised_renderer_test
	cargo test --package kernel --lib --features semantic_surface_transport_test
	cargo test --package kernel --lib --features semantic_surface_presentation_test
	cargo test --package kernel --lib --features desktop_dev
	cargo test --package kernel --lib --features workspace_dev
	cargo test --package kernel --lib --features owned_workspace_test
	cargo test --package kernel --lib --features owned_workspace_cpu_test
	cargo test --package kernel --lib --all-features
	cargo test --package xtask
	cargo xtask supervised-renderer-inspect
	cargo xtask supervised-renderer-all-features-link
	cargo xtask semantic-surface-transport-inspect
	cargo xtask semantic-surface-transport-all-features-link
	cargo xtask semantic-presentation-inspect
	cargo xtask semantic-presentation-all-features-link
	cargo xtask normal-exclusion
	cargo xtask workspace-image
	cargo xtask owned-workspace-cpu-inspect
	cargo xtask dispatcher-image
	cargo xtask owned-dispatcher-image
	cargo xtask application-profile-image
	cargo xtask mixed-workspace-build
	cargo xtask native-editor-workspace-build
	cargo xtask native-application-image
	cargo xtask build-native-runtime
	cargo xtask native-durable-failure-build
	cargo xtask native-repeated-build
	cargo xtask native-interactive-build
	cargo xtask build-native-document
	cargo xtask build-elf-document
	cargo xtask build-native-relaunch
	cargo xtask build-elf-relaunch
	cargo xtask elf-application-image
	cargo xtask build-linux-application
	cargo xtask build-shared-veil
	cargo xtask build-linux-tls
	cargo xtask build-linux-loader-entry
	cargo xtask build-veil-application
	cargo clippy --package anomalous-abi --all-targets -- -D warnings
	cargo clippy --package anomalous-editor-contract --all-targets -- -D warnings
	cargo clippy --package anomalous-editor-contract --all-targets --features remote_editor -- -D warnings
	cargo clippy --package anomalous-editor-contract --target x86_64-unknown-none --features remote_editor -- -D warnings
	cargo clippy --package anomalous-native-runtime --all-targets -- -D warnings
	cargo clippy --package anomalous-program-manifest --all-targets -- -D warnings
	cargo clippy --package anomalous-program-manifest --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-document-protocol --all-targets -- -D warnings
	cargo clippy --package anomalous-document-protocol --all-targets --features remote_workspace -- -D warnings
	cargo clippy --package anomalous-elf-adapter --all-targets -- -D warnings
	cargo clippy --package anomalous-elf-adapter --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-elf-adapter --all-targets --features linux_image -- -D warnings
	cargo clippy --package anomalous-elf-adapter --target x86_64-unknown-none --features linux_image -- -D warnings
	ANOMALOUS_ELF_DEPLOYMENT="$(CURDIR)/build/elf-application/deployment.elf" cargo clippy --package anomalous-elf-loader --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-runtime-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-program-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-document-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-relaunch-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none --features failure_proof -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none --features repeated_proof -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none --features interactive_proof -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none --features remote_workspace_proof -- -D warnings
	cargo clippy -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem --package kernel --features native_interactive_editor_test --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-native-durable-client --target x86_64-unknown-none --all-features -- -D warnings
	cargo clippy --package anomalous-linux-client --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-linux-veil --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-linux-veil --target x86_64-unknown-none --features tls -- -D warnings
	cargo clippy --package anomalous-linux-veil --target x86_64-unknown-none --features loader-entry -- -D warnings
	cargo clippy --package anomalous-linux-client --target x86_64-unknown-none --features tls -- -D warnings
	cargo clippy --package anomalous-veil-protocol --all-targets --all-features -- -D warnings
	cargo clippy --package anomalous-veil-protocol --target x86_64-unknown-none --features linux-policy -- -D warnings
	cargo clippy --package anomalous-veil-protocol --target x86_64-unknown-none --features linux-tls-policy -- -D warnings
	cargo clippy --package anomalous-veil-protocol --target x86_64-unknown-none --features read-only-files -- -D warnings
	cargo clippy --package anomalous-veil-protocol --target x86_64-unknown-none --features source-read -- -D warnings
	cargo clippy --package anomalous-desktop-theme --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-theme --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --all-targets --features native_editor_bridge -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --target x86_64-unknown-none --features native_editor_bridge -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --all-targets --features native_editor_remote -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --target x86_64-unknown-none --features native_editor_remote -- -D warnings
	cargo clippy --package anomalous-window-surface --all-targets -- -D warnings
	cargo clippy --package anomalous-window-surface --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --all-targets --features native_editor_bridge -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --target x86_64-unknown-none --features native_editor_bridge -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --all-targets --features native_editor_remote -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --target x86_64-unknown-none --features native_editor_remote -- -D warnings
	cargo clippy --package anomalous-workspace-service --lib -- -D warnings
	cargo clippy --package anomalous-workspace-service --lib --features native_remote -- -D warnings
	cargo clippy --package anomalous-desktop-workspace --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-desktop-compositor --target x86_64-unknown-none -- -D warnings
	cargo clippy -Zjson-target-spec -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem --release --package anomalous-workspace-service --target targets/x86_64-anomalous-workspace.json --locked --no-default-features -- -D warnings
	cargo clippy -Zjson-target-spec -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem --release --package anomalous-workspace-service --target targets/x86_64-anomalous-workspace.json --locked --no-default-features --features owned_cpu_probe -- -D warnings
	cargo clippy -Zjson-target-spec -Zbuild-std=core,compiler_builtins -Zbuild-std-features=compiler-builtins-mem --release --package anomalous-workspace-service --target targets/x86_64-anomalous-workspace.json --locked --no-default-features --features native_remote -- -D warnings
	cargo clippy --package anomalous-desktop-renderer --all-targets --all-features -- -D warnings
	cargo clippy --package anomalous-desktop-scene --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-service --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-recovery --all-targets -- -D warnings
	cargo clippy --package anomalous-desktop-recovery --target x86_64-unknown-none -- -D warnings
	cargo clippy --package anomalous-desktop-transport --all-targets --all-features -- -D warnings
	cargo clippy --package anomalous-session --all-targets -- -D warnings
	cargo clippy --package anomalous-session-router --all-targets -- -D warnings
	cargo clippy \
		-Z build-std=core,compiler_builtins \
		-Z build-std-features=compiler-builtins-mem \
		--package kernel \
		--all-features \
		--target x86_64-unknown-none \
		-- -D warnings
	cargo clippy \
		-Zjson-target-spec \
		-Z build-std=core,compiler_builtins \
		-Z build-std-features=compiler-builtins-mem \
		--release \
		--package anomalous-semantic-surface-source \
		--target targets/x86_64-anomalous-semantic-surface.json \
		--locked \
		--no-default-features \
		-- -D warnings
	cargo clippy \
		-Zjson-target-spec \
		-Z build-std=core,compiler_builtins \
		-Z build-std-features=compiler-builtins-mem \
		--release \
		--package anomalous-semantic-surface-consumer \
		--target targets/x86_64-anomalous-semantic-surface.json \
		--locked \
		--no-default-features \
		-- -D warnings
	cargo clippy \
		-Zjson-target-spec \
		-Z build-std=core,compiler_builtins \
		-Z build-std-features=compiler-builtins-mem \
		--release \
		--package anomalous-semantic-surface-renderer \
		--target targets/x86_64-anomalous-semantic-surface.json \
		--locked \
		--no-default-features \
		-- -D warnings
	cargo clippy \
		-Zjson-target-spec \
		-Z build-std=core,compiler_builtins \
		-Z build-std-features=compiler-builtins-mem \
		--release \
		--package anomalous-supervised-renderer \
		--target targets/x86_64-anomalous-renderer.json \
		--locked \
		--no-default-features \
		-- -D warnings
	cargo clippy --package xtask --all-targets -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features desktop_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_workspace_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_workspace_cpu_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_workspace_cpu_test,owned_dispatcher_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_workspace_test,owned_dispatcher_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features dispatcher_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features dispatcher_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_application_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_runtime_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_runtime_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_program_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_program_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_failure_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_repeated_editor_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_failure_test,native_repeated_editor_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_repeated_editor_test,native_relaunch_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_failure_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_failure_test,native_relaunch_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_failure_test,elf_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_test,native_relaunch_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_durable_test,elf_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_document_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_document_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_document_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_document_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_relaunch_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_relaunch_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_relaunch_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_relaunch_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features elf_application_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_application_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_tls_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_loader_entry_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_loader_entry_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_loader_entry_test,linux_tls_test,shared_veil_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_loader_entry_test,application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_loader_entry_test,native_editor_workspace_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_tls_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_tls_test,shared_veil_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_tls_test,application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features linux_tls_test,native_editor_workspace_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features shared_veil_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features shared_veil_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features veil_application_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features veil_application_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features ny_object_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features ny_object_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features mixed_workspace_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features native_editor_workspace_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features mixed_workspace_test,ny_object_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features application_profile_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_dispatcher_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_multi_region_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_multi_region_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_multi_region_test,application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_source_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_source_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_source_test,application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_tls_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_tls_test,workspace_dev -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_tls_test,application_profile_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_tls_test,owned_source_test -- -D warnings
	cargo clippy --package kernel --target x86_64-unknown-none --features owned_dispatcher_test,workspace_dev -- -D warnings
	bash -n scripts/run-virtualbox.sh
	python3 -B -m unittest discover -s scripts -p test_virtualbox_proof.py

clean:
	cargo clean
	rm -f build/*.bin build/*.debug build/*.elf build/*.img build/*.log
	rm -f build/*.o build/*.png build/*.ppm

.PHONY: native-durable-build native-durable-smoke native-durable-failure-build native-durable-failure-smoke
native-durable-build:
	cargo xtask native-durable-build
native-durable-smoke:
	cargo xtask native-durable-smoke
native-durable-failure-build:
	cargo xtask native-durable-failure-build
native-durable-failure-smoke:
	cargo xtask native-durable-failure-smoke

.PHONY: native-repeated-build native-repeated-smoke
native-repeated-build:
	cargo xtask native-repeated-build
native-repeated-smoke:
	cargo xtask native-repeated-smoke

.PHONY: native-interactive-build native-interactive-smoke
native-interactive-build:
	cargo xtask native-interactive-build
native-interactive-smoke:
	cargo xtask native-interactive-smoke

.PHONY: mixed-workspace-build mixed-workspace-smoke
mixed-workspace-build:
	cargo xtask mixed-workspace-build
mixed-workspace-smoke:
	cargo xtask mixed-workspace-smoke

.PHONY: native-editor-workspace-build native-editor-workspace-smoke
native-editor-workspace-build:
	cargo xtask native-editor-workspace-build
native-editor-workspace-smoke:
	cargo xtask native-editor-workspace-smoke

.PHONY: ny-vessel-inspect
ny-vessel-inspect:
	cargo xtask ny-vessel-inspect

.PHONY: ny-vessel-image ny-vessel-smoke
ny-vessel-image:
	cargo xtask ny-vessel-image

ny-vessel-smoke:
	cargo xtask ny-vessel-smoke
