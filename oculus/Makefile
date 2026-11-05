.PHONY: check clean hello smoke scene-smoke manager-smoke compositor-smoke input-smoke appearance-smoke session-smoke

python ?= python3

# The shared, project-wide Anomalous Ny runtime lives at the repository root.
# Refresh it with ../scripts/refresh-ny-runtime.sh (or `make -C ../eidolon sdk`).
NY_RUNTIME_DIR ?= ../runtime
NY_COMPILER ?= $(NY_RUNTIME_DIR)/nycompiler
NY_INTERPRETER ?= $(NY_RUNTIME_DIR)/nyinterpreter
NY_STDLIB ?= $(NY_RUNTIME_DIR)/nystd
NY_SRC ?= src

hello_output := Oculus: hello from the shared Anomalous runtime

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
	"$(NY_INTERPRETER)" build/hello.nil > /tmp/oculus-hello-out 2>&1
	@printf 'oculus hello smoke passed\n'

# --- Scene model (Unit 2) ---

build/scene-smoke.nil: src/oculus/scene/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

scene-smoke: build/scene-smoke.nil
	"$(NY_INTERPRETER)" build/scene-smoke.nil

# --- Window management (Unit 3) ---

build/manager-smoke.nil: src/oculus/scene/manager-smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

manager-smoke: build/manager-smoke.nil
	"$(NY_INTERPRETER)" build/manager-smoke.nil

# --- Compositor (Unit 4) ---

build/compositor-smoke.nil: src/oculus/compositor/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

compositor-smoke: build/compositor-smoke.nil
	"$(NY_INTERPRETER)" build/compositor-smoke.nil

# --- Input routing (Unit 5) ---

build/input-smoke.nil: src/oculus/input/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

input-smoke: build/input-smoke.nil
	"$(NY_INTERPRETER)" build/input-smoke.nil

# --- Appearance / theme (Unit 6) ---

build/appearance-smoke.nil: src/oculus/appearance/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

appearance-smoke: build/appearance-smoke.nil
	"$(NY_INTERPRETER)" build/appearance-smoke.nil

# --- Integration and session (Unit 7) ---

build/session-smoke.nil: src/oculus/session/smoke.mat
	mkdir -p build
	"$(NY_COMPILER)" --no-bundle \
		-I "$(NY_STDLIB)" \
		-I "$(NY_SRC)" \
		--output $@ $<

session-smoke: build/session-smoke.nil
	"$(NY_INTERPRETER)" build/session-smoke.nil

check: smoke scene-smoke manager-smoke compositor-smoke input-smoke appearance-smoke session-smoke appearance-smoke input-smoke

clean:
	rm -rf build
