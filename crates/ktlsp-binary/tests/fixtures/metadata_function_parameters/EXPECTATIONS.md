# Frozen pre-execution expectations

Frozen contract seal f30e3c4d123eb8e4f7aa591ca10983f14ab1724f34fe9a32dcefd61e46a9c81e.
Exact tuples/errors/prefixes/minima are independently source-derived and ported
into metadata_function_parameters.rs; no new decoder supplied its own goldens.
All9 fixtures,184 prefix rows,23 base errors with3 prefix variants,13 competing
limits,9 signed scalar representations across4 child/3 parent fields, ordered
growth1..5 and hard clamps were frozen before execution. S means
size_of::<SelectedParameter>(), not a hardcoded ABI size.

| Fixture | Parent (flags,old_flags,name) | Parameter body spans | Records | Work | Owned bytes |
| --- | --- | --- | --- | --- | --- |
| minimal | [None, None, 0] | [] | 2 | 9 | 0 |
| selected | [None, None, 0] | [[4, 6]] | 5 | 26+S | S |
| duplicates | [None, None, 4] | [[4, 6], [10, 12]] | 12 | 59+2S | 2S |
| wrong_wire | [None, None, 0] | [[20, 22]] | 11 | 70+S | S |
| opaque | [None, None, 0] | [[7, 18]] | 12 | 79+S | S |
| signed | [0, 0, -1] | [[12, 32]] | 12 | 110+S | S |
| nonminimal | [None, None, 0] | [[8, 11]] | 5 | 37+S | S |
| pair_copy | [65734, None, 17] | [[20, 28], [30, 38]] | 18 | 149+2S | 2S |
| pair_equals | [66006, None, 19] | [[8, 16]] | 10 | 70+S | S |

Parent complete framing/required-name errors precede child failures. All spans
and final errors are Function-relative; child faults rebase once. Valid wrong
wire field6 values stay raw. Empty selected body fails Parameter(MissingName)
after its node charge and before vector reservation; empty-parent raw readers
retain their old MissingName behavior. Function/Parameter/Wire stage wrappers
are distinct. k4 work-minus-one fails child dispatch as Parameter(Wire(work))
@16; k1/2/3/5 fail outer reservation @2/6/10/18. Owned0 has no minus-one case.

Named child count21844 costs records65534,bytes87378,nodes21845,depth2,
work371357+32768S,owned32768S. Count21845 fails outer selection record admission
at87378; no per-child budget reset/refund. Failed node/record/owned admission
followed by work failure preserves the prior charge. Logical capacity excludes
allocator surplus, churn, stack and RSS. Other pipeline stages retain separate
budgets. Finite hostile204800 cases check determinism/bounds/borrowing, not
universal correctness or performance. All execution remains pending.
