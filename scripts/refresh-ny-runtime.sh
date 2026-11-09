#!/usr/bin/env bash
# Refresh the shared Anomalous Ny runtime.
#
# Every Anomalous sub-project (eidolon, oculus, and future ones) compiles and
# runs Ny through one project-wide runtime. This script builds a complete,
# validated Ny SDK from the sibling Ny checkout and copies the resolved
# generation into a real `runtime/` directory, so no sub-project depends on a
# symlink into another checkout.
#
# Private, generated state lives under `.runtime/`:
#   .runtime/sdk            the builder's atomically swapped activation symlink
#   .runtime/store/versions/immutable SDK generations (the builder's store)
#
# The CMake build itself defaults to the Ny checkout's `build/` directory
# (`NY_BUILD_DIR`) so a refresh reuses the development build incrementally.
#
# `runtime/` and `.runtime/` are both Git-ignored and never committed.
#
# Environment overrides:
#   NY_REPO_DIR            Compatible SDK source checkout (required explicitly)
#   NY_BUILD_DIR           CMake build root (default: <ny_repo>/build)
#   NY_RUN_TESTS           run the Ny test suite (default: ON)
#   NY_ALLOW_DIRTY_SOURCE  permit an explicitly marked development SDK (default: OFF)
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
anomalous_root="$(cd "${script_dir}/.." && pwd -P)"
if [[ -z "${NY_REPO_DIR:-}" ]]; then
    printf 'Set NY_REPO_DIR to a compatible SDK source checkout. See docs/optional-runtime.md.\n' >&2
    exit 2
fi
ny_repo_dir="${NY_REPO_DIR}"

if [[ ! -x "${ny_repo_dir}/scripts/build-sdk.sh" ]]; then
    printf 'Ny SDK builder is missing or not executable: %s\n' \
        "${ny_repo_dir}/scripts/build-sdk.sh" >&2
    exit 2
fi

runtime_dir="${anomalous_root}/runtime"
state_dir="${anomalous_root}/.runtime"
store_dir="${state_dir}/store"
builder_link="${state_dir}/sdk"
build_root="${NY_BUILD_DIR:-${ny_repo_dir}/build}"

mkdir -p "${state_dir}"

# The consumer-facing runtime is a real directory, not an indirection.
if [[ -L "${runtime_dir}" ]]; then
    printf 'Refusing a symlinked Ny runtime path: %s\n' "${runtime_dir}" >&2
    exit 2
fi

# Build and validate a complete generation. The Ny builder publishes the
# generation behind an atomically replaced symlink inside the private state
# directory; we then copy the resolved generation into the shared runtime.
NY_ALLOW_DIRTY_SOURCE="${NY_ALLOW_DIRTY_SOURCE:-OFF}" \
NY_RUN_TESTS="${NY_RUN_TESTS:-ON}" \
NY_BUILD_DIR="${build_root}" \
NY_SDK_DIR="${builder_link}" \
NY_SDK_STORE_DIR="${store_dir}" \
    "${ny_repo_dir}/scripts/build-sdk.sh"

if [[ ! -L "${builder_link}" || ! -d "${builder_link}" ]]; then
    printf 'Ny SDK activation is not a live symlink: %s\n' "${builder_link}" >&2
    exit 2
fi

resolved_generation="$(readlink -f "${builder_link}")"
case "${resolved_generation}/" in
    "${store_dir}/"*) ;;
    *)
        printf 'Ny SDK resolved outside the runtime generation store: %s\n' \
            "${resolved_generation}" >&2
        exit 2
        ;;
esac

next_runtime="${runtime_dir}.next.$$"
rm -rf "${next_runtime}"
mkdir -p "${next_runtime}"

cleanup() {
    if [[ -n "${next_runtime:-}" && -d "${next_runtime}" ]]; then
        if [[ "${next_runtime}" == "${runtime_dir}.next."* ]]; then
            rm -rf -- "${next_runtime}"
        else
            printf 'Refusing to clean an unexpected runtime staging path: %s\n' \
                "${next_runtime}" >&2
        fi
    fi
}
trap cleanup EXIT

cp -a "${resolved_generation}/." "${next_runtime}/"

# The generation store is private (mode 0700); the shared runtime tree must be
# traversable and readable by every local sub-project.
chmod -R go+rX "${next_runtime}"

required_tools=(
    nilcompiler
    nilinterpreter
    nilassembler
    nillinker
    nillsp
    nildebugger
    nilschema
    nilagent
    nilbuild
    aperture
    niltest
)

for tool in "${required_tools[@]}"; do
    if [[ ! -x "${next_runtime}/${tool}" ]]; then
        printf 'Ny runtime tool is missing or not executable: %s\n' "${tool}" >&2
        exit 2
    fi
done

for directory in nillib nilstd docs; do
    if [[ ! -d "${next_runtime}/${directory}" ]]; then
        printf 'Ny runtime directory is missing: %s\n' "${directory}" >&2
        exit 2
    fi
done

if [[ ! -f "${next_runtime}/manifest.json" ]]; then
    printf 'Ny runtime manifest is missing\n' >&2
    exit 2
fi
freestanding_runtime="${next_runtime}/nillib/native/libnilruntime_freestanding.a"
if [[ ! -f "${freestanding_runtime}" ]]; then
    printf 'Ny runtime has no freestanding native runtime: %s\n' \
        "${freestanding_runtime}" >&2
    exit 2
fi
if [[ -e "${next_runtime}/bin" ]]; then
    printf 'Ny runtime has a forbidden bin/ indirection\n' >&2
    exit 2
fi

retired_runtime_name="ny""vm"
if find -L "${next_runtime}" -type f -name "${retired_runtime_name}*" -print -quit | grep -q .; then
    printf 'Ny runtime contains a retired runtime alias\n' >&2
    exit 2
fi

while IFS= read -r -d '' source; do
    relative="${source#"${next_runtime}/nilstd/"}"
    binary="${next_runtime}/nillib/${relative%.ny}.nil"
    if [[ ! -f "${binary}" ]]; then
        printf 'Ny runtime has no compiled module for %s\n' "${relative}" >&2
        exit 2
    fi
done < <(find -L "${next_runtime}/nilstd" -type f -name '*.ny' -print0)

python3 "${ny_repo_dir}/scripts/validate-sdk.py" \
    "${next_runtime}" \
    --expected-revision "$(git -C "${ny_repo_dir}" rev-parse --verify HEAD)"

# Swap the real runtime/ directory in with a bounded rollback window.
previous_runtime="${state_dir}/previous-runtime.$$"
rm -rf "${previous_runtime}"
if [[ -e "${runtime_dir}" ]]; then
    mv "${runtime_dir}" "${previous_runtime}"
fi
if ! mv "${next_runtime}" "${runtime_dir}"; then
    if [[ -e "${previous_runtime}" ]]; then
        mv "${previous_runtime}" "${runtime_dir}"
    fi
    printf 'Failed to activate the refreshed Ny runtime\n' >&2
    exit 1
fi
rm -rf "${previous_runtime}"
next_runtime=""

printf 'Anomalous Ny runtime refreshed at %s\n' "${runtime_dir}"
printf 'Ny source revision: %s\n' "$(git -C "${ny_repo_dir}" rev-parse --verify HEAD)"
printf 'Freestanding native runtime: %s\n' \
    "${runtime_dir}/nillib/native/libnilruntime_freestanding.a"
