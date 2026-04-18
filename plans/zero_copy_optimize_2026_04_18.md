# Slice-pointer `Buffer` refactor — eliminate the heap mirror of mmap

## Context

`MmkvImpl::new` (`src/core/mmkv_impl.rs:50-56`) iterates the entire mmap at startup and decodes every record into an owned `Buffer = Arc<KV>` (`src/core/buffer.rs:15-16`). Each `KV` carries the key as `String`, the value as `Vec<u8>`, plus `buffa` framing overhead. The map currently lives as `Arc<RwLock<HashMap<String, Buffer>>>` (`src/core/shared_state.rs`) and **all reads serve from this heap copy — the mmap is never touched at read time**. So every byte exists twice (once on disk via mmap, once on the heap), defeating the point of mmap.

Goal: the in-memory map stores only `(offset, length)` pointers into the current mmap; reads parse on demand directly out of mmap. Public API (`MMKV::get<T>` returning owned `T`) is unchanged so FFI/JNI surfaces aren't touched.

## Approach

1. **Use `buffa`'s zero-copy `KVView<'a>`** (already supported by the `buffa = "0.2"` dependency, just disabled in `build.rs`). At init, decode each record into a `KVView`, compute `(value_offset, value_len)` by pointer arithmetic against the mmap base, and store those instead of an owned `KV`. At read, `KVView::decode_view(&mmap[record_range])` borrows directly from mmap — no allocation for unencrypted reads.

2. **`Buffer` becomes a two-variant enum**:
   - `Owned { kv: Arc<KV>, seq: u64 }` — transient, used between `put` and the async IO flush. Holds the bytes on the heap because they're not in mmap yet.
   - `Slice(SliceLoc)` — points into the current mmap. `SliceLoc` is `cfg`-split: CRC mode = `{ type_token, value_offset, value_len }`, encryption mode = `{ type_token, record_offset, record_len, position }` (encryption stores the ciphertext range plus the AEAD counter; decrypt-then-`KVView::decode_view` happens at read time).

3. **Writer-thread promotes `Owned → Slice` after every successful append**, freeing the heap. To avoid losing a newer put when the writer's promotion races with a same-key second put, attach a monotonic `seq` to every `Owned`; promotion only swaps in `Slice` if the current map entry still has the seq the writer was working on. Otherwise drop the promotion and let the queued newer write handle it.

4. **Share the mmap slab via `ArcSwap<MmapHandle>`** instead of `RwLock<MemoryMap>`:
   - Extract `Arc<RawMmap>` (already a struct in `src/core/memory_map.rs:19-23`); wrap as `MmapHandle { raw: Arc<RawMmap> }` exposing `read(Range) -> &[u8]`.
   - Writer keeps the canonical `MemoryMap` for mutation (only writer mutates).
   - `SharedState { mmap: ArcSwap<MmapHandle>, kv_map: RwLock<HashMap<String, Buffer>> }`. Writer publishes a fresh `Arc<MmapHandle>` after `ensure_capacity` (writer.rs:118) and after trim. Plain `append` doesn't swap (slab pointer doesn't move; bytes are visible via shared `Arc<RawMmap>`).
   - Reads: `let mmap = state.mmap.load(); let buf = state.kv_map.read().get(key).cloned();` — no lock on the mmap path.

5. **Shadow-file trim** replaces the in-place `IOWriter::rewrite_snapshot` (`src/core/writer.rs:92-104`):
   - Snapshot `kv_map`. Build `<file>.tmp.<random>` with the rewritten content (rotating nonce first if encrypted, mirroring today's `before_rewrite`).
   - `fsync` → atomic `rename` → mmap the new file → `state.mmap.store(new_handle)`.
   - Build a fresh `HashMap` whose `Slice` entries point at the new mmap (preserving any concurrently-arrived `Owned` entries verbatim). Atomically swap `kv_map` (`*state.kv_map.write() = new_map`).
   - Crash-safety bonus: atomic rename means partial trim can never corrupt the live file.
   - Concurrent `put`s arriving during trim land as `Owned` in the current map; their queued IO jobs run after trim completes (single IO thread serializes everything via `IOLooper`), then promote into the new mmap.

## Files to modify

- `build.rs` — flip `.generate_views(false)` → `.generate_views(true)`.
- `Cargo.toml` — add `arc-swap = "1"`.
- `src/core/memory_map.rs` — extract `Arc<RawMmap>`; add `MmapHandle` with `read(Range) -> &[u8]`.
- `src/core/shared_state.rs` — replace `SharedKvMap` alias with `pub struct SharedState { mmap: ArcSwap<MmapHandle>, kv_map: RwLock<HashMap<String, Buffer>> }`.
- `src/core/buffer.rs` — turn `Buffer` into the two-variant enum; rework `parse<T>` to take `&MmapHandle` and use `KVView::decode_view`; drop `key()`, `value()`, `shared_with()`; change `Encoder::encode_to_bytes` to take `(key: &str, type_token: i32, value: &[u8], position: u32)` instead of `&Buffer` so trim can re-encode without re-materializing a `KV`.
- `src/core/iter.rs` — `into_map` returns `Slice`-valued entries; pass mmap base for offset arithmetic. For tombstones, decode with `KVView` and remove the key.
- `src/core/crc.rs` and `src/core/encrypt.rs` — `decode_bytes` returns the record range (and `position` for encryption) so init can construct `Slice` directly. Encryption decode-on-read path lives in `Buffer::parse` (decrypt → `KVView::decode_view` → `T::from_bytes`).
- `src/core/mmkv_impl.rs` — init builds `Slice` map via the new iterator; `put` allocates a `seq` (`AtomicU64`) and inserts `Owned`; `get` loads `state.mmap`, fetches buffer, calls `parse(&mmap)`.
- `src/core/writer.rs` — after `mm.append`, take `kv_map.write()`, seq-check, promote `Owned → Slice`. Replace `rewrite_snapshot` with shadow-file trim.

## Execution order (each step compiles + existing tests pass)

1. **Plumbing.** Enable `KVView` in `build.rs`. Add `arc-swap` dep. Introduce `MmapHandle` and `SharedState` (mmap unused by readers yet). Tests unchanged.
2. **Enum split.** Make `Buffer` an enum with only the `Owned` variant initially active; thread `&MmapHandle` through `parse`/`Encoder`. All construction still produces `Owned`. Tests unchanged.
3. **Init builds `Slice`.** Switch `iter::into_map` to `KVView`-based `Slice` construction. Existing trim/expand tests still assert on `mm.write_offset()` byte counts — verify those still match.
4. **Promotion handshake.** Add `seq: u64` to `Owned`; writer promotes after append with seq-check. Add a regression test for the put→put→promote race.
5. **Shadow-file trim.** Replace `rewrite_snapshot` with the new path. Verify `trim_rotates_nonce`, `trim_rewrites_from_shared_snapshot_after_delete`, `trim_reads_latest_shared_snapshot`, `test_trim_and_expand_*` still pass. Add a crash test (kill between rename and unlink).
6. **Cleanup.** Remove now-unused `Buffer::key`, `value`, `shared_with`, `from_encoded_bytes` (if any callers remain, replace with direct `KVView` calls).

## Tests likely to need updates

- `test_buffer_clone_is_shallow` (buffer.rs:447) — `Arc::ptr_eq` no longer applicable to `Slice`; rewrite or drop.
- `impl PartialEq for Buffer` (buffer.rs:359) — gate behind `#[cfg(test)]` and only compare `Owned` variants.
- `IOWriter` test constructors (writer.rs:182-443) — now take `Arc<SharedState>` instead of `SharedKvMap`.
- `iter::tests::test_mmap_iterator` — adapt to new `into_map` signature.

Tests that should pass unchanged: `test_trim_and_expand_default`/`_encrypt`, `test_multi_thread_mmkv`, `test_sync_visibility_for_put_and_delete`, `test_post_failure_rolls_back_shared_state`, `test_reopen_recovers_previous_nonce_after_interrupted_rotation`, all `MemoryMap` tests, `test_crypt_buffer`, `test_rotate_nonce_changes_ciphertext`, `test_recover_current_nonce_restores_previous_generation`.

## Verification

- `cargo test` (CRC mode) and `cargo test --features encryption` both green.
- New regression test: 4 threads × 1000 puts each, with one thread alternating put/delete on a shared key. After drop+reopen, every key has its last-written value. (Strengthens existing `test_multi_thread_mmkv`.)
- New regression test: walks the put(K,V1) → writer-appends-V1 → put(K,V2) → writer-promotes sequence in a controlled order (use `IOLooper::call` to step the IO thread) and asserts `get(K) == V2`.
- New crash test: invoke trim, kill the process between `rename` and old-mmap drop (use a fault-injection hook), reopen, verify all keys readable.
- Microbenchmark: load a 100k-entry mmap and measure RSS before/after init. Expected: roughly `sum(key_bytes) + 32B/entry` instead of today's `sum(key_bytes + value_bytes) + ~150B/entry`.

## Risk

Medium-high. The shadow-file trim and the `seq`-checked promotion both add new race-condition surfaces. ~3-day refactor for someone fluent in this codebase; budget 5 days if encryption nonce-rotation interaction with shadow-file trim needs hardening. Estimated diff: +650 / -240 lines across 9 files plus tests.

## Confirmed scope (from planning Q&A)

- Promotion: **Full** — init builds `Slice`, writer promotes `Owned → Slice` after each append.
- Trim: **Shadow-file** — write to `<file>.tmp`, fsync, atomic rename; reads never block.
- Encryption: **Both modes refactored** — encrypted `Slice` stores `(record_offset, record_len, position)`; decrypt-on-read. RAM win, slower reads, no plaintext cached at rest.
