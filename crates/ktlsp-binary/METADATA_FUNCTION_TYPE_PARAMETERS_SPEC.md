# T018/#18 selected Function TypeParameter composition

Frozen source base5657b7cf12247f3c16f8c2ed2672f686b266a1de,
tree99b3602c82b265ddc64a254890d9654320b189c8. Architect approved bounded scope.
Decode one caller-selected unframed Function for supported mv=[2,3,0]. Version,
kind/container selection remains the caller's job. Return FunctionHeader plus
one ordered Vec<SelectedTypeParameter { span, parameter: TypeParameter }>.
Only field4/wire2 is selected. Preserve duplicates, raw wrong-wire/unknown fields,
scalar presence/last-recognized behavior; no merging or lookup. Bounds remain
opaque, including malformed nested bytes. Nested bound spans remain child-relative;
outer spans and every public error are Function-relative. Public structs are
caller-constructible, not independently validated objects.

Length preflight, one string_table::Budget and root node(1,0), then COMPLETE
function_header::decode_with_budget. Only after parent framing/required name passes,
perform a charged second wire scan (including dispatch work for ignored fields).
Selected body gets node(2,start), unchanged type_parameter::decode_with_budget
using the SAME whole budget, then outer budget.push at tag start. Rebase child
errors once by body start. ErrorKind::{Wire,Function,TypeParameter} preserves origin.
No partial output, refunds, recursion or new helper/dependency. Child vectors and
outer vector share owned/work allowances; packed entries share record/work.

Success accounting: N/F=parent bytes/fields, ni/fi=child bytes/fields, Pi/Li/Ui=
packed entries/bytes/unpacked IDs, C=all logical retained capacity reservations:
records=2F+sum(fi+Pi), nodes=1+m, depth=1 if m0 else2,
work=1+2N+4F+sum(1+ni+2fi+Li+2Pi+Ui)+C, owned=C.
Existing hard clamps and deterministic power-of-two logical reservations apply;
owned excludes allocator surplus/churn, stack and RSS. No whole-pipeline bound.

Source allowlist: new module, one additive metadata/mod.rs export, focused test,
new metadata_function_type_parameters fixture directory and this spec/review note.
Existing helper APIs/callers/tests, Cargo/lock/workflows/server/global status stay
unchanged. No server consumer/provider, semantics, bound expansion, type/name
resolution, generics or parity claim. Positive coverage authored-only GAP.
Local execution and compile held under #96; pinned fmt allowed. Original unchanged
hosted Linux checks are the executor after actual source reviews and publication.
Parents #18/#68 remain OPEN; T008/T008a HOLD, C13/B3/B4 GAP, B11 UNVERIFIED,
#7/#83/#96 OPEN and Darwin/Windows UNKNOWN remain.
