# Current Rust and Java execution boundary

The user clarified both questions on 2026-10-12: an isolated official JVM oracle is permitted, and the Rust server may invoke the user's existing JDK/build tools for optional Gradle/Maven project import, build, run and debug. No Java runtime/JDK is bundled.

The server implementation, native build and core editor analysis remain Rust-only. No JVM/JDK is required for standalone source editing, parsing, native semantics or the default native test suite. No embedded compiler, JNI analyzer, C/C++ parser or shipped Java/Kotlin analysis helper. Reading class/source archives and Kotlin metadata is data processing in Rust. Kotlin fixture source is test data.

Optional external tooling is explicit: use configured/existing Java, Gradle, Maven and project wrappers, report unavailable tools clearly, preserve user environment/settings, support cancellation and failure reporting, and distinguish model provenance. Do not claim a static build-file reader reconstructs arbitrary executable Gradle/Maven models. Implement native orchestration of external tools as separate small tickets. Debugger interoperability may connect to a user-owned JVM target; target execution does not make the Rust language engine JVM-based.

The user-authorized isolated official oracle can use its own JVM distribution in research storage. That distribution and scripts never enter the native release/archive/dependency graph. Native CI remains runnable without Java. Optional external-tool integration/oracle tests are separately identified.

This clarification supersedes earlier documents' prohibition on external JDK invocation. It does not authorize replacing the native semantic engine with a JVM/compiler process or bundling Java to regain parity.
