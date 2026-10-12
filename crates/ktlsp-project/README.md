# Explicit native project model

`load_manifest(path)` reads a user-selected JSON manifest; `from_json(path, bytes)` parses already available bytes using the manifest parent as the path base. Both return a validated `ProjectModel` or a named `ProjectError`. The default discovery name `ktlsp.json` belongs to later server integration.

The schema is defined in `docs/specs/project-model.md`. Public serializable structs preserve module IDs, source/test/generated roots, dependency scope/export, library references, SDK archives and language settings. Module library references are compile/non-exported in v1. Library and module ID namespaces are separate; duplicate references within either namespace are rejected.

Paths are native-platform absolute paths normalized lexically. Missing binaries, sources and SDK archives remain model data. The loader does not invoke Gradle/Maven/JPS, run a compiler, download or inspect artifacts, discover source files, or resolve symlinks. Lexically disjoint roots can refer to the same directory through symlinks; a separate filesystem discovery/ownership check is required before analysis. Language/API/JVM version strings receive syntax/order validation only, not support validation.

Before typed model allocation, a streaming JSON pass enforces aggregate collection budgets: 4,096 modules, 8,192 libraries, 65,536 module dependency and module library reference edges combined, 16,384 roots, and 32,768 binary/source/SDK archive paths combined. Inputs are limited to 2 MiB; file reading uses at most that size plus one sentinel byte. JSON's default nesting limit remains enabled. Graph validation is iterative; lexical root ownership uses sorting and an ancestor stack rather than comparing every pair.
