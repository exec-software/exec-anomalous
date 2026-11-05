# Matrix Language Issues

Found during Archon development. Each entry: the observation, what was
expected, and the current state.

---

## 1. `ny.lang.text::split_lines` is unusable from Matrix

**Observed:** `ny.lang.text::split_lines` returns
`ny::lang::collections::vec<text>`. A Matrix page cannot treat that value as an
`array of text`: `the length of lines` fails with
`Type 'ny::lang::collections::vec<text>' has no member 'len'`, and assigning it
to a field declared `field lines as array of text` fails with
`Type mismatch in struct field 'lines'. Expected 'array<text>', got
'ny::lang::collections::vec<text>'`.

**Expected:** A Ny function returning a sequence of tier-one values should be
consumable from Matrix, or the compiler should convert the `vec` at the
boundary, since Matrix's own collection literal is `array of text`.

**Cause:** Matrix's `array` and Ny's `collections::vec` are distinct NIL types;
the surface languages share a semantic core but not a collection
representation.

**Workaround:** `archon.engine.patterns::split-lines` implements line splitting
in Matrix using `ny.lang.text::char_at`, `substring` and `index_of_from`, all of
which take and return tier-one types.

**Status:** Noted. This is the reason the engine carries its own splitter.

---

## 2. Value-position `consider` demands a wildcard arm even when exhaustive

**Observed:** A value-position `consider` whose arms cover every variant of a
`choice` fails code generation with
`match expression requires a final unguarded wildcard arm (add '_ -> value' so
every path produces a result)`.

**Expected:** An exhaustive match over a closed `choice` should not require an
unreachable `otherwise`.

**Cause:** Code generation raises the requirement before exhaustiveness is
known, rather than after.

**Workaround:** `archon.model.severity`, `confidence` and `category` add an
`otherwise give <default>` arm. The default is unreachable but harmless.

**Status:** Noted. The error message is precise and names the fix.

---

## 3. A Ny `i64` returning function flows into Matrix `number`

**Observed:** `ny.lang.text::index_of`/`count` return `i64`, and
`int_to_string_i64` takes `i64`. Matrix `number` values pass to them and their
results assign to `number` without an explicit `becomes`.

**Expected:** Since Matrix `number` is documented as 64-bit, this interoperation
is reasonable and desirable.

**Status:** Working as intended; recorded so the boundary is not surprising.

---

## 4. A book's declared name must match its file path

**Observed:** A rule book declared `book archon.rules.matrix` lived in
`src/archon/rules/matrix-codestyle.mat`. Pages that did
`read everything from archon.rules.matrix` failed with `Undefined identifier`
for every function the book defines, at each call site — not a "module not
found" diagnostic.

**Expected:** Either the import resolves from the declared book name, or the
compiler reports that the module could not be located.

**Cause:** The module resolver maps a dotted book name to a file path under the
include roots; a file whose path does not mirror the book name is not found, and
the failure surfaces later as unresolved identifiers.

**Workaround:** Name the file to mirror the book exactly
(`book archon.rules.matrix` lives in `src/archon/rules/matrix.mat`).

**Status:** Noted. Cost real time during the CLI work; worth a clearer
diagnostic.

---

## 5. `value` is a reserved word and cannot be a binding or parameter

**Observed:** Writing `int_to_string_i64 with value being x` fails with
`expected a value, found 'value'`, and using `value` as a `consider` arm binding
fails with `expected 'then' after a 'consider' arm pattern, found 'value'`.

**Expected:** Reserved words are rejected as identifiers with a clear message.

**Cause:** `value` introduces a binding declaration, so it is reserved.

**Workaround:** Use another name. `ny.lang.text::int_to_string_i64`'s parameter
is `n`, so the label is `n`; `consider` arms bind names like `item` or `source`.

**Status:** Working as intended; the errors are precise once the cause is known.

---

## 6. A Ny module can span a whole directory

**Observed:** `runtime/nystd/ny/lang/io/file.ny`, `path.ny`, `checksum.ny` and
others all declare `module ny.lang.io`, so the import is
`read everything from ny.lang.io`, not `ny.lang.io.file` (there is no
`file.ny`-named submodule).

**Expected:** Reading the directory depth as the module path is tempting but
wrong: the module name is declared in the file, not derived from the tree.

**Status:** Noted so the import spelling is not guessed.

---

## 7. `argv` excludes the program name; `main`'s return is the exit code

**Observed:** Running `nyinterpreter page.nil foo bar` yields
`arg_count` = 2 and `arg with index being 0` = `foo`; the program name is not in
the arguments. A `main returns number` value becomes the process exit code
(`give 2` exits 2).

**Expected:** Either convention is defensible; this one is convenient for a CLI
because every argument is a target.

**Status:** Working as intended; recorded so the CLI's contract is explicit.

