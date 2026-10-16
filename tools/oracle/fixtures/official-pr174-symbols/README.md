# PR174 official-capture preparation inputs

These are unchanged copies of `docs/specs/lowering-acceptance/declarations.kt`
and `error-node.kt` from PR174 (`44f77b9`), with the reviewed eight-message
script. They are source data, not official observations or expected symbol
responses. The script opens each exact UTF-8 source and requests document symbols
sequentially, then shutdown/exit. It adds no readiness requests or replies to
unexpected server requests.

| Input | Bytes | SHA256 |
| --- | ---: | --- |
| declarations.kt | 146 | fa935b144a49838fd9e603bfc95462bcf3701eeb19877501d451ca069ce9e233 |
| error-node.kt | 44 | c052b78e7603a88f3531f5b06f3a533f8df5394ce8ce9eeab3eb2b3ce3f4bd2c |
| script.json | 2351 | 930a87ea1fa9c3f6e6a6b6008d010dd9435082329ea8b897a42084d7369ab008 |

The input validator compares actual input bytes with embedded bytes.
Supplied archive/launcher/hash claims are compared with pins, not authenticated.
Consistent proposed identity/preflight data validates successfully, granting no
launch authority. No output or process is created. No Java/archive is included.
