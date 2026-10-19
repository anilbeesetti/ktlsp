# Current Rust and Java execution boundary

The user clarified both questions on 2026-10-12: an isolated official JVM oracle is permitted, and the Rust server may invoke the user's existing JDK/build tools for optional Gradle/Maven project import, build, run and debug. No Java runtime/JDK is bundled.

The server implementation, native build and core editor analysis remain Rust-only. No JVM/JDK is required for standalone source editing, parsing, native semantics or the default native test suite. No embedded JVM compiler, JNI analyzer, C/C++ analysis dependency or shipped Java/Kotlin analysis helper. Reading class/source archives and Kotlin metadata is data processing in Rust. Kotlin fixture source is test data.

Optional external tooling is explicit: use configured/existing Java, Gradle, Maven and project wrappers, report unavailable tools clearly, preserve user environment/settings, support cancellation and failure reporting, and distinguish model provenance. Do not claim a static build-file reader reconstructs arbitrary executable Gradle/Maven models. Implement native orchestration of external tools as separate small tickets. Debugger interoperability may connect to a user-owned JVM target; target execution does not make the Rust language engine JVM-based.

The user-authorized isolated official oracle can use its own JVM distribution in research storage. That distribution and scripts never enter the native release/archive/dependency graph. Native CI remains runnable without Java. Optional external-tool integration/oracle tests are separately identified.

This clarification supersedes earlier documents' prohibition on external JDK invocation. It does not authorize replacing the native semantic engine with a JVM-hosted compiler process or bundling Java to regain parity.

The user-authorized Krusty adoption (#446) reuses a pinned Rust compiler frontend
inside a supervised native analysis worker, not a JVM compiler process.
Its extraction excludes backend/codegen, `stacker`/`psm` and C/native stack helpers;
Rust resource guards and process containment replace the latter without claiming
equivalent arbitrary-recursion acceptance. The first LSP consumer is explicit
standalone pull diagnostics with core builtins only. It does not turn existing
data-only T004/project/metadata APIs into compiler/classpath semantics.


Explicit JSON source authority (#421) adds bounded read-only Rust filesystem
inventory beside, not inside, existing data-only JSON/JPS/Maven/T004 producers.
Only configured local projects are discovered; unknown URI queries do not probe
outside membership. Windows-only `file-id = 0.2.3` supplies native OS identity,
not C/C++ analysis or Java. No JDK/compiler/build tool is invoked for this path.
Capture-bearing composition passed original full three-host runtime and independent
actual-value/edit gates. Capture-free source then passed its OWN reviews/full
three-host CI, guarded PR422 integration and both original exact-main gates.
[Exact evidence and limits](../status/development.md#verified422-capture-free-integration-and-original-exact-main-acceptance)
do not establish full official import, effective classpaths or external-tool
orchestration. #96 local server/parser/Session/worker restrictions still apply.

