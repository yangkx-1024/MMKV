# MMKV test suite

Two kinds of tests, one home for each kind of assertion.

| Where | Scope |
| --- | --- |
| `src/**/mod tests` | Unit tests for items that need private access: encoding primitives, file sizing, framing, the IO looper, record iteration, the memory map, the writer's offsets, `MmkvImpl` internals, the instance cache and the C API. |
| `src/core/test_support.rs` | `#[cfg(test)] pub(crate)` helpers shared by the unit tests (temp `Config`, `MmkvImpl` open/reopen, the real encoder, measured record lengths, raw header surgery, trim blockers). Not reachable from `tests/`. |
| `tests/common/mod.rs` | Helpers shared by the integration tests. Public API + `tempfile` + std only. |
| `tests/api.rs` | Public API behaviour: every value type, edge values, type changes, missing keys, custom types, persistence, `clear_data`. |
| `tests/durability.rs` | The write contract: a write reaches the file before it returns, a failed write rolls back, values survive a drop and a reopen, overwrites trim instead of growing. |
| `tests/corruption.rs` | Damaged files through the public API: crafted headers and frames, appended garbage, a byte-flip sweep, and (encryption) a damaged, missing or mismatched key/meta file. |
| `tests/concurrency.rs` | Threads: concurrent writers, readers during a trim, several handles on one directory. |
| `tests/crash.rs` | Crash consistency: `SIGKILL` a child process mid-trim, reopen, check every acknowledged key survived and the shadow file was swept. |
| `tests/model.rs` | Randomized model-based test driven by a deterministic RNG; a `HashMap` mirrors the expected state. |

## Conventions

- **Both flavours must pass.** The default build frames records with CRC-8, `--features encryption` with AES-EAX. Use `#[cfg(feature = "encryption")]` only where the assertion is genuinely feature-specific; the encryption key lives inside `common::Store::open`.
- **Size pages from a measured record length**, never from a hard-coded 17 or 24 bytes. `common::record_len(key, value)` writes the record with the real encoder of the current build and reports its exact on-disk size; `src` tests use `test_support::record_len` / `page_for`.
- **Never write into the repo root.** Every store lives in a `tempfile::TempDir` (`common::Store`), which takes the data file, the `.meta` file and any trim tmp file with it when it drops.
- **No `std::thread::sleep`.** Synchronise with joins, with the synchronous `put`/`delete` contract, or by dropping handles. `tests/crash.rs` is the one exception and says why: the writer runs a trim synchronously inside `put`, so a kill placed between two `put` calls can never land inside one. It waits on the clock only to choose *where* to kill; the child reports through a marker file that the keys under test are already written, and every assertion holds wherever the kill lands.
- **Handles share one instance per directory** while any of them is alive. Drop every handle to force a reopen from disk.
- **Process-wide state**: `MMKV::set_log_level` / `set_logger` affect the whole binary. `Store::new()` lowers the level to `Warn` once via `std::sync::Once`; tests inside one binary run in parallel threads, so each test builds its own `Store`.
- Test names are descriptive snake_case phrased as behaviour, with no `test_` prefix. Every file starts with a `//!` line stating its scope.

## Running

```sh
cargo test                                             # default flavour
cargo test --features encryption                       # AES-EAX flavour
cargo test --test api --test durability                # one binary at a time
cargo test --test crash                                # spawns child processes and kills them
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features encryption -- -D warnings
cargo fmt --check
```

Keep each integration binary under ~10 s and the whole `cargo test` under ~30 s per flavour in debug.
