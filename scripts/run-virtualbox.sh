#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 0 ]]; then
    echo "usage: $0" >&2
    exit 2
fi

if ! command -v VBoxManage >/dev/null 2>&1; then
    echo "VirtualBox is required, but VBoxManage is not on PATH" >&2
    exit 1
fi

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_path="$repository_root/build/anomalous.img"
vdi_path="$repository_root/build/anomalous.vdi"
default_vm_name="Anomalous Dev [Threshold]"
vm_mode="${ANOMALOUS_VM_MODE:-normal}"
vm_name="${ANOMALOUS_VM_NAME:-$default_vm_name}"
vm_start="${ANOMALOUS_VM_START:-1}"
controller="Anomalous SATA"

case "$vm_mode" in
    normal)
        serial_log="$repository_root/build/virtualbox.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-gui}"
        ;;
    desktop)
        image_path="$repository_root/build/desktop-dev.img"
        vdi_path="$repository_root/build/desktop-dev.vdi"
        serial_log="$repository_root/build/virtualbox-desktop.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-gui}"
        ;;
    workspace)
        image_path="$repository_root/build/workspace-dev.img"
        vdi_path="$repository_root/build/workspace-dev.vdi"
        serial_log="$repository_root/build/virtualbox-workspace.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-gui}"
        ;;
    control)
        serial_log="$repository_root/build/virtualbox-control.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-headless}"
        feature_name="control"
        passed_event='"event":"kernel.control_test.passed"'
        failed_event='"event":"kernel.control_test.failed"'
        verifier_command="verify-control-transcript"
        verifier_log="build/virtualbox-control.log"
        ;;
    scheduler)
        serial_log="$repository_root/build/virtualbox-scheduler.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-headless}"
        feature_name="scheduler"
        passed_event='"event":"kernel.scheduler_test.passed"'
        failed_event='"event":"kernel.scheduler_test.failed"'
        verifier_command="verify-scheduler-transcript"
        verifier_log="build/virtualbox-scheduler.log"
        ;;
    supervision)
        serial_log="$repository_root/build/virtualbox-supervision.log"
        vm_frontend="${ANOMALOUS_VM_FRONTEND:-headless}"
        feature_name="supervision"
        passed_event='"event":"kernel.supervision_test.passed"'
        failed_event='"event":"kernel.supervision_test.failed"'
        verifier_command="verify-supervision-transcript"
        verifier_log="build/virtualbox-supervision.log"
        ;;
    *)
        echo "ANOMALOUS_VM_MODE must be normal, desktop, workspace, control, scheduler, or supervision" >&2
        exit 2
        ;;
esac

if [[ "$vm_start" != "0" && "$vm_start" != "1" ]]; then
    echo "ANOMALOUS_VM_START must be 0 or 1" >&2
    exit 2
fi

if [[ "$vm_mode" != "normal" && "$vm_mode" != "desktop" && "$vm_mode" != "workspace" && "$vm_name" != "$default_vm_name" ]]; then
    echo "$vm_mode mode must use VirtualBox VM '$default_vm_name'" >&2
    exit 2
fi

feature_timeout_seconds="${ANOMALOUS_VM_TIMEOUT_SECONDS:-30}"
if [[ "$vm_mode" != "normal" && "$vm_mode" != "desktop" && "$vm_mode" != "workspace" ]] &&
    { [[ ! "$feature_timeout_seconds" =~ ^[1-9][0-9]*$ ]] ||
        ((10#$feature_timeout_seconds > 300)); }; then
    echo "ANOMALOUS_VM_TIMEOUT_SECONDS must be an integer from 1 through 300" >&2
    exit 2
fi

if [[ ! -f "$image_path" ]]; then
    echo "missing $image_path; run 'make image' (normal/proof) or 'make desktop-image' / 'make workspace-image' first" >&2
    exit 1
fi

if VBoxManage showvminfo "$vm_name" --machinereadable >/dev/null 2>&1; then
    state="$(VBoxManage showvminfo "$vm_name" --machinereadable | sed -n 's/^VMState="\(.*\)"/\1/p')"
    if [[ "$state" != "poweroff" && "$state" != "aborted" ]]; then
        echo "VirtualBox VM '$vm_name' must be powered off (current state: $state)" >&2
        exit 1
    fi
else
    VBoxManage createvm --name "$vm_name" --ostype Other_64 --register
fi

VBoxManage modifyvm "$vm_name" \
    --firmware bios \
    --memory 512 \
    --cpus 2 \
    --vram 32 \
    --graphicscontroller vmsvga \
    --boot1 disk \
    --boot2 none \
    --boot3 none \
    --boot4 none \
    --nic1 none \
    --uart1 0x3F8 4 \
    --uart-mode1 file "$serial_log"

if ! VBoxManage showvminfo "$vm_name" --machinereadable | grep -Fq '="Anomalous SATA"'; then
    VBoxManage storagectl "$vm_name" --name "$controller" --add sata --controller IntelAhci --portcount 1 --bootable on
fi

# Refresh only the medium attached to the dedicated Anomalous controller.
VBoxManage storageattach "$vm_name" \
    --storagectl "$controller" \
    --port 0 \
    --device 0 \
    --type hdd \
    --medium none >/dev/null 2>&1 || true
if [[ -e "$vdi_path" ]]; then
    if VBoxManage showmediuminfo disk "$vdi_path" >/dev/null 2>&1; then
        VBoxManage closemedium disk "$vdi_path" --delete
    else
        rm -f -- "$vdi_path"
    fi
fi
VBoxManage convertfromraw "$image_path" "$vdi_path" --format VDI
VBoxManage storageattach "$vm_name" \
    --storagectl "$controller" \
    --port 0 \
    --device 0 \
    --type hdd \
    --medium "$vdi_path"

echo "Serial events: $serial_log"
if [[ "$vm_start" == "0" ]]; then
    echo "VM configured and left powered off because ANOMALOUS_VM_START=0"
    exit 0
fi

if [[ "$vm_mode" != "normal" ]]; then
    # A stale pass record must never satisfy a later feature run.
    rm -f -- "$serial_log"
fi

echo "Starting '$vm_name' through the Anomalous Threshold loader"
if [[ "$vm_mode" == "desktop" || "$vm_mode" == "workspace" ]]; then
    # The launcher emits only static input/command metadata. Keep the file
    # private nonetheless; future diagnostics must not become ambient output.
    (umask 077; : > "$serial_log")
    chmod 600 "$serial_log"
fi
VBoxManage startvm "$vm_name" --type "$vm_frontend"

if [[ "$vm_mode" == "normal" || "$vm_mode" == "desktop" || "$vm_mode" == "workspace" ]]; then
    exit 0
fi

read_vm_state() {
    VBoxManage showvminfo "$vm_name" --machinereadable |
        sed -n 's/^VMState="\(.*\)"/\1/p'
}

power_off_feature_vm() {
    local attempt
    local state
    state="$(read_vm_state)"
    if [[ "$state" == "poweroff" || "$state" == "aborted" ]]; then
        return 0
    fi

    VBoxManage controlvm "$vm_name" poweroff >/dev/null
    for ((attempt = 0; attempt < 100; attempt++)); do
        state="$(read_vm_state)"
        if [[ "$state" == "poweroff" || "$state" == "aborted" ]]; then
            return 0
        fi
        sleep 0.1
    done

    echo "VirtualBox VM '$vm_name' did not reach poweroff after $feature_name mode" >&2
    return 1
}

feature_waiting=1
stop_feature_on_exit() {
    local status=$?
    trap - EXIT INT TERM
    if [[ "$feature_waiting" == "1" ]]; then
        power_off_feature_vm || true
    fi
    exit "$status"
}
stop_feature_on_signal() {
    exit 130
}
trap stop_feature_on_exit EXIT
trap stop_feature_on_signal INT TERM

deadline=$((SECONDS + feature_timeout_seconds))
terminal_seen=0
while ((SECONDS < deadline)); do
    if [[ -f "$serial_log" ]]; then
        if grep -Fq "$passed_event" "$serial_log"; then
            terminal_seen=1
            break
        fi
        if grep -Fq "$failed_event" "$serial_log" ||
            grep -Eq '"level":"(error|fatal)"' "$serial_log"; then
            terminal_seen=1
            break
        fi
        boot_entries="$(grep -Fc '"event":"boot.kernel.enter"' "$serial_log" || true)"
        stage_2_banners="$(grep -Fc 'Threshold stage 2' "$serial_log" || true)"
        if ((boot_entries > 1 || stage_2_banners > 1)); then
            terminal_seen=1
            break
        fi
    fi

    state="$(read_vm_state)"
    if [[ "$state" != "running" && "$state" != "starting" ]]; then
        break
    fi
    sleep 0.1
done

power_off_feature_vm
feature_waiting=0
trap - EXIT INT TERM

if [[ "$terminal_seen" != "1" ]]; then
    echo "$feature_name mode did not reach a terminal record within ${feature_timeout_seconds}s; inspect $serial_log" >&2
    exit 1
fi

(
    cd "$repository_root"
    cargo xtask "$verifier_command" "$verifier_log"
)
echo "${feature_name^} transcript passed strict validation: $serial_log"
