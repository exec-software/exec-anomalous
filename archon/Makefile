.PHONY: check clean hello smoke model-smoke engine-smoke scanner-cli cli-smoke

python ?= python3

# The shared, project-wide Anomalous Ny runtime lives at the repository root.
# Refresh it with ../scripts/refresh-ny-runtime.sh (or `make -C ../eidolon sdk`).
NY_RUNTIME_DIR ?= ../runtime
NY_COMPILER ?= $(NY_RUNTIME_DIR)/nycompiler
NY_INTERPRETER ?= $(NY_RUNTIME_DIR)/nyinterpreter
NY_STDLIB ?= $(NY_RUNTIME_DIR)/nystd
NY_SRC ?= src

hello_output := Archon: hello from the shared Anomalous runtime

# --- Hello page (Unit 1) ---

build/hello.nil: src/hello.mat
	@test -x "$(NY_COMPILER)" || { \
		printf 'shared Ny runtime missing at %s\n' "$(NY_RUNTIME_DIR)" >&2; \
		printf 'run scripts/refresh-ny-runtime.sh from the repository root\n' >&2; \
		exit 1; \
	}
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle -I "$(NY_STDLIB)" --output $@ $<

hello: build/hello.nil
	"$(NY_INTERPRETER)" build/hello.nil

smoke: build/hello.nil
	"$(NY_INTERPRETER)" build/hello.nil > /tmp/archon-hello-out 2>&1
	@grep -qxF "$(hello_output)" /tmp/archon-hello-out
	@printf 'archon hello smoke passed\n'

# --- Finding semantic model (Unit 2) ---

build/model-smoke.nil: src/archon/model/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

model-smoke: build/model-smoke.nil
	"$(NY_INTERPRETER)" build/model-smoke.nil > /tmp/archon-model-out 2>&1
	@cat /tmp/archon-model-out
	@grep -q 'kind=Unbounded copy' /tmp/archon-model-out
	@grep -q 'level=high' /tmp/archon-model-out
	@grep -q 'domain=code' /tmp/archon-model-out
	@printf 'archon model smoke passed\n'

# --- Rule engine and first scan (Unit 3) ---

build/engine-smoke.nil: src/archon/engine/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

engine-smoke: build/engine-smoke.nil
	"$(NY_INTERPRETER)" build/engine-smoke.nil > /tmp/archon-engine-out 2>&1
	@cat /tmp/archon-engine-out
	@grep -q 'target=engine/sample.c findings=3 highest=critical rules=3' /tmp/archon-engine-out
	@printf 'archon engine smoke passed\n'

# --- Standalone scanner CLI (Unit 4) ---

build/scanner-cli.nil: src/archon/cli/scanner-cli.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

scanner-cli: build/scanner-cli.nil
	"$(NY_INTERPRETER)" build/scanner-cli.nil $(SCAN_ARGS)

# Scan the shipped insecure fixture and assert its machine-readable report.
cli-smoke: build/scanner-cli.nil
	"$(NY_INTERPRETER)" build/scanner-cli.nil testdata/insecure.mat > /tmp/archon-cli-out 2>&1
	@cat /tmp/archon-cli-out
	@grep -qxF 'critical: Hard-coded secret at testdata/insecure.mat:14:0' /tmp/archon-cli-out
	@grep -qxF 'high: Inline assembly at testdata/insecure.mat:8:0' /tmp/archon-cli-out
	@grep -qxF 'target=testdata/insecure.mat findings=6 highest=critical rules=15' /tmp/archon-cli-out
	@printf 'archon cli smoke passed\n'

check: smoke model-smoke engine-smoke cli-smoke

clean:
	rm -rf build

