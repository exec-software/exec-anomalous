# Matrix Language Issues

Found during Oculus development. Each entry: the observation, what was expected, and the current state.

---

## 1. Cross-book record method symbol mangling (FIXED)

**Observed:** Methods declared in an extension on a user-defined record in a
different book were unreachable at run time, even though compilation succeeded.

**Cause:** The method symbol was mangled from the extending book instead of the
record's declaring book. Example:

```
declared  oculus__window__geometry__util__rectangle__area   ← extending book
called    oculus__window__geometry__rectangle__area          ← record's book
```

**Expected:** Methods should resolve through the type's declaring book regardless
of where the extension lives.

**Fixed:** Pushed to master. All three sites now resolve through one helper that
asks where the type is actually declared.

**Verification:** `matrix.execution` test suite now includes a cross-book
extension fixture that exercises the fix.

---

## 2. `say` on a boolean prints `1`/`0`, not `true`/`false`

**Observed:** `say some-boolean` prints `1` or `0`.

**Expected:** A language whose surface is prose should print `true` / `false`.

**Status:** Left alone in the fix for the mangling bug; worth a decision later.

---

## 3. Book path segments with `-`

**Observed:** `book oculus.window-geometry-rectangle` compiles but the `-` is
rewritten to `_` in the emitted NIL symbol, flattening the hierarchy.

**Expected:** Book path segments should not contain `-`; use `.` for namespace
separation. `book oculus.window.geometry.rectangle` gives proper hierarchical
namespacing.

**Status:** Noted for awareness.

---

## 4. Cross-book type imports fail silently (FIXED)

**Observed:** `read everything from` did not import record types from another
book. Compilation of the importing book succeeded but the type was invisible to
it.

**Cause:** The import resolver only looked for `.ny` files, not `.mat`/`.matrix`
files. A book is a NIL module, so it should resolve the same way regardless of
the source surface.

**Fixed:** `.matrix` and `.mat` now join `.ny` in the candidate list for
include-path resolution.

**Verification:** The scene-node book now compiles with only its entry page on
the command line, pulling in window/geometry, window/role, window/focus, and
window/identity all through `read everything from`.

---

## 5. `read everything from` could not find files with hyphenated names (FIXED)

**Observed:** A book named `oculus.window.geometry-rectangle-ext` could not be
found by `read everything from` even though the file existed under the include
root.

**Cause:** The lookup built the filename from the module name
(`geometry_rectangle_ext.mat`), not the author-written hyphenated spelling.

**Fixed:** Both spellings (hyphenated and underscored) are now tried.

**Verification:** The geometry-rectangle-ext book resolves correctly from
scene/smoke through its transitive dependency chain.

---

## 6. Value-position `when` vs statement-position `when` asymmetry (FIXED)

**Observed:** A `when` used in value position (`value x is when cond give A
otherwise give B end when`) required `give` after the condition, while the
same `when` as a statement required `then`. The error message was precise but
the asymmetry was a real learning cost for a cold model.

**Fixed:** The compiler now accepts both forms in value position, so `when
cond then give A otherwise give B end when` and `when cond give A otherwise
give B end when` both work.

**Status:** Friction removed.

---

## 7. Constructing an undeclared type was silent (FIXED)

**Observed:** `build rectangle with ...` where nothing declares `rectangle`
compiled, linked, and ran — exit zero. The only symptom was a later "has no
member 'width'" error.

**Cause:** Ny's structural literal check didn't verify the type existed before
accepting field names.

**Fixed:** Now an error naming the real fault, first.

---

## 8. "Unknown type" error didn't explain why (FIXED)

**Observed:** "Unknown type 'point' in struct field" was accurate but read as a
broken import to someone whose file compiles in isolation.

**Fixed:** The error now says "Nothing in scope declares it; if it is declared
elsewhere, that file must also be given to the compiler."

---

## 9. Classes could not be extended (FIXED)

**Observed:** `X begin extension` on a class failed with "has no member" — in
another book and in the class's own.

**Cause:** Member lookup returned as soon as the class declaration didn't
recognise a name, never reaching the table where extensions register. The
attach pass also dropped extensions whose target lives in another book.

**Fixed:** Member lookup now reaches the extension table; attach pass now
handles cross-book class extensions.

**Status:** Classes can now be extended. Critical for a window manager built on
classes.

---

## 10. `read everything from` imports declarations, not types (BY DESIGN)

**Observed:** `read everything from` imports paragraphs and functions but types
are only visible if the declaring file is compiled together with the consumer.

**Expected:** Types should be imported like declarations.

**Status:** Not a bug — Matrix uses a one-compilation-then-link model: a book
and everything it reads go to the compiler together. The `read` statement
expresses a dependency, not a copy. The compiler's transitive resolution
handles it automatically when all files are under the include root.

**Lesson:** Only the entry page goes on the command line; everything else is
pulled in through `-I`.

---

## 11. Choice variants need `when Name` (no `then`)

**Observed:** Choice declarations with bare variants (e.g. `Unmapped`) or with
`then` (e.g. `when Normal then`) fail. The compiler expects `when Name` only.

**Expected:** Choice syntax is consistent across all types.

**Lesson:** Use `when Name` — no `then`, no bare identifiers.

---

## 12. Reserved word `Tab` in choice variants

**Observed:** `Tab` is a reserved word in Matrix, even inside choice variants.
`when Tab` fails with MX0101.

**Expected:** Reserved words should be escapable inside choices.

**Workaround:** Use `Tab-key` or backtick escape.

---

## 13. Nested `build` calls confuse field boundaries

**Observed:** `build pointer-state with cursor-position being build point with
x being 0, y being 0, active-button being 0` interprets `active-button` as a
field of the inner `build point` rather than the outer `build pointer-state`.

**Expected:** The compiler should scope fields to their nearest enclosing struct
literal.

**Workaround:** Split into separate `value` bindings — one for the inner struct,
then one for the outer.

---

## 14. Reserved word `text` used as variable name

**Observed:** `value text is build color-value ...` fails with MX0101. `text` is
a reserved type name.

**Expected:** The same word is a valid type reference but not a variable name.

**Workaround:** Use `text-color` or `text-val`.
