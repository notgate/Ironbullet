# Architecture restructuring plan

This plan governs the GitHub-facing cleanup of Ironbullet after the initial pipeline correctness repairs. It separates internal refactors from serialized pipeline compatibility so saved `.rfx`, `.svb`, `.opk`, and `.loli` imports are not silently broken.

## Evidence baseline

- `BlockType` is a serialized import/export contract, not just a palette enum.
- The native block engine explicitly rejects `Script` and `WebSocket`; multipart request bodies are also explicitly rejected pending a real schema.
- The frontend contains a separate manual block catalog and type union. This has already diverged from Rust: `NuDataSensor` exists natively but has no typed GUI editor, while previously exposed unavailable block kinds had to be removed from the creation palette.
- The Rust code generator contains TODO-output paths for some block kinds. It must not be treated as a complete exporter until those paths have a declared capability contract.
- Runner construction previously crossed the GUI, job, CLI, and worker boundaries through positional dependency lists. This was converted to `RunnerSetup` and `WorkerRuntime` in `97b8c28`.

## Removal contract

A model or function may be deleted only when all of the following are true:

1. It is absent from the native execution dispatch, frontend creation and settings routes, imports, exports, plugins, and IPC.
2. It is not part of the persisted pipeline schema or a supported legacy import mapping.
3. A repository search confirms there is no runtime call path.
4. The headless core suite, frontend production build, and relevant import fixtures pass after removal.

If a serialized block is obsolete but still appears in historical inputs, preserve deserialization behind a `LegacyImportOnly` capability until a documented migration removes it deliberately.

## Target layout

```text
src/
  app/                 # application boot and desktop shell composition
  ipc/                 # command routing and feature handlers
  pipeline/
    model/             # serialized pipeline, variables, settings, capabilities
    engine/            # execution services grouped by domain
    import/            # format adapters and migration rules
    export/            # format adapters and code-generation capability checks
  runner/              # named runtime setup, scheduler, worker, output
  sidecar/             # sidecar protocol and transport adapters
gui/src/lib/
  domain/              # pipeline/runner data contracts
  features/            # editor, runner, inspector, settings, plugins
  shared/              # reusable primitives, IPC client, formatting
  catalog/             # generated/derived visible-block presentation data
docs/site/             # Starlight source and public capability reference
```

Moves must use `git mv` and preserve module visibility boundaries. Do not rename serialized enum variants or JSON field names during file organization.

## Phased implementation

### Phase 1 — capability source of truth

- Add a native capability record for every `BlockType`: category, label, creation visibility, execution status, import compatibility, and code-generation status.
- Expose the capability list through a read-only IPC command.
- Replace the handwritten frontend creation catalog with the capability payload plus a frontend-only icon map.
- Keep unavailable and legacy-import-only blocks readable in existing configurations, but exclude them from new-block creation.
- Add a contract test asserting every serializable block type has an explicit capability record.

### Phase 2 — model and naming consolidation

- Move block settings into `pipeline/model/settings/{request,parse,check,function,control,browser,protocol,utility}.rs`.
- Rename generic or ambiguous helpers only when callers and test names move together.
- Make public visibility intentional: `pub` only for cross-crate/IPC/serialization APIs, `pub(crate)` for application internals, private otherwise.
- Preserve serde aliases for any user-visible setting rename.

### Phase 3 — execution and exporter boundaries

- Split `engine/mod.rs` dispatch into domain-specific executors with one shared execution context.
- Split `export/rust_codegen/block_codegen.rs` by supported domains.
- Replace generated TODO stubs with an explicit export capability report. A pipeline containing unsupported export blocks must fail export clearly instead of producing deceptively incomplete Rust.
- Add local loopback fixtures for each newly claimed supported transport behavior.

### Phase 4 — GUI feature boundaries

- Move the editor, runner, inspector, settings, and plugin-builder components into feature folders.
- Co-locate settings component, settings type, capability status, and serialization adapter per block domain.
- Remove duplicated frontend type definitions only after IPC/type contract tests prove the native model is the source of truth.

### Phase 5 — deletion and release hygiene

- Delete only capability records marked unavailable with no legacy import requirement and no plugin/IPC reachability.
- Update README, Starlight capability status, changelog, and migration notes in the same commit.
- Run: `cargo fmt -- --check`, `cargo test --lib --no-default-features`, `cd gui && npm run build`, `cd docs/site && npm run build`, `git diff --check`.

## Known non-goals for the first migration

- Do not re-enable Script, WebSocket, or multipart creation merely to preserve catalog counts.
- Do not expose `NuDataSensor` in the GUI until its settings/editor contract and execution tests are complete.
- Do not delete anti-bot, import, browser, or protocol models from serialized configurations based only on current GUI reachability.
