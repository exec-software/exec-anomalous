.PHONY: check check-ny check-ny-native clean decode encode inspect ny-cli ny-native-cli sdk test test-reference

python ?= python3
# The shared, project-wide Anomalous Ny runtime lives at the repository root.
# Refresh it with `make sdk` (or scripts/refresh-ny-runtime.sh) from the root.
NY_RUNTIME_DIR ?= ../runtime
NY_COMPILER ?= $(NY_RUNTIME_DIR)/nycompiler
NY_INTERPRETER ?= $(NY_RUNTIME_DIR)/nyinterpreter
NY_ASSEMBLER ?= $(NY_RUNTIME_DIR)/nyassembler
NY_LINKER ?= $(NY_RUNTIME_DIR)/nylinker
NY_STDLIB ?= $(NY_RUNTIME_DIR)/nystd
NY_RUNTIME ?= $(NY_RUNTIME_DIR)/nylib/native/libnyruntime.a

check: test check-ny
	PYTHONPYCACHEPREFIX=build/pycache $(python) -m py_compile reference/*.py tests/*.py
	bash -n ../scripts/refresh-ny-runtime.sh

test:
	PYTHONDONTWRITEBYTECODE=1 $(python) -m unittest discover -s tests -v

test-reference:
	PYTHONDONTWRITEBYTECODE=1 $(python) -m unittest -v tests.test_codec

sdk:
	../scripts/refresh-ny-runtime.sh

check-ny:
	test -x "$(NY_COMPILER)"
	test -x "$(NY_INTERPRETER)"
	test -x "$(NY_ASSEMBLER)"
	test -x "$(NY_LINKER)"
	test -d "$(NY_STDLIB)"
	NY_COMPILER="$(NY_COMPILER)" NY_INTERPRETER="$(NY_INTERPRETER)" \
		NY_ASSEMBLER="$(NY_ASSEMBLER)" NY_LINKER="$(NY_LINKER)" \
		NY_STDLIB="$(NY_STDLIB)" \
		PYTHONDONTWRITEBYTECODE=1 $(python) -m unittest discover \
		-s tests -p 'test_ny*.py' -v

check-ny-native:
	test -x "$(NY_COMPILER)"
	test -x "$(NY_INTERPRETER)"
	test -x "$(NY_ASSEMBLER)"
	test -x "$(NY_LINKER)"
	test -d "$(NY_STDLIB)"
	NY_COMPILER="$(NY_COMPILER)" NY_INTERPRETER="$(NY_INTERPRETER)" \
	NY_ASSEMBLER="$(NY_ASSEMBLER)" NY_LINKER="$(NY_LINKER)" \
	NY_STDLIB="$(NY_STDLIB)" \
	PYTHONDONTWRITEBYTECODE=1 $(python) -m unittest -v \
		tests.test_ny_codec.ny_codec_tests.test_llvm_jit_matches_the_interpreted_oracle \
		tests.test_ny_codec.ny_codec_tests.test_llvm_aot_matches_the_interpreted_oracle \
		tests.test_ny_native_cli

ny-cli:
	test -x "$(NY_COMPILER)"
	test -x "$(NY_INTERPRETER)"
	test -d "$(NY_STDLIB)"
	mkdir -p build/ny
	"$(NY_COMPILER)" --no-stdlib --sdk-path "$(NY_STDLIB)" --no-bundle --quiet \
		--output build/ny/eidolon-cli.text.nil \
		src/cli.ny src/format.ny src/ppm.ny src/codec.ny src/limits.ny
	"$(NY_COMPILER)" --no-stdlib --sdk-path "$(NY_STDLIB)" --no-bundle --quiet \
		--emit-binary-nil --output build/ny/eidolon-cli.binary.nil \
		src/cli.ny src/format.ny src/ppm.ny src/codec.ny src/limits.ny

ny-native-cli: ny-cli
	test -x "$(NY_ASSEMBLER)"
	test -x "$(NY_LINKER)"
	test -f "$(NY_RUNTIME)"
	"$(NY_ASSEMBLER)" -o build/ny/eidolon-cli.o \
		build/ny/eidolon-cli.text.nil
	"$(NY_LINKER)" --runtime "$(NY_RUNTIME)" -o build/ny/eidolon \
		build/ny/eidolon-cli.o

encode:
	@echo 'usage: python3 -m reference.cli encode OUTPUT.eidolon FRAME.ppm [...]'

decode:
	@echo 'usage: python3 -m reference.cli decode INPUT.eidolon OUTPUT_DIRECTORY'

inspect:
	@echo 'usage: python3 -m reference.cli inspect INPUT.eidolon'

clean:
	find reference tests -type d -name __pycache__ -prune -exec rm -rf {} +
