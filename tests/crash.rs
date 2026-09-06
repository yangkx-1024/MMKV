//! Crash consistency: `SIGKILL` a real process while it is rewriting the store, then
//! reopen the file in the parent and check that every record the child had already
//! acknowledged is still readable.
//!
//! These tests knowingly break two conventions from `tests/README.md`. They spawn a
//! process (the test binary re-executes itself with `--exact`) and they wait on the
//! clock. Neither is avoidable here: the writer runs a shadow-file trim synchronously
//! inside `put`, so a kill placed between two `put` calls can never land in the middle
//! of one. Only the *kill point* is timing-dependent; every assertion below holds no
//! matter where the kill lands, and the child announces through a marker file that the
//! keys under test are already written, so the assertions never race with the child.
//!
//! `Child::kill` sends `SIGKILL`, so the child runs no destructor, no `Drop`, and no
//! atexit handler. Whatever the parent then finds on disk was put there by the writes
//! themselves, which is exactly the state a power loss or an OOM kill would leave.

mod common;

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use common::{DATA_FILE_NAME, Store};
use mmkv::Error::KeyNotFound;

/// Directory the child's store lives in. Set by the parent, absent in a normal run.
const DIR_VAR: &str = "MMKV_CRASH_DIR";

/// The child creates this once every key that must survive is in the file.
const READY_FILE: &str = "ready";

/// Small keys the child writes before it signals readiness. Every one of them must be
/// readable after the kill, whichever side of the trim the kill landed on.
const STATIC_KEYS: usize = 12;

/// The key the child rewrites in a loop to force back-to-back trims.
const HOT_KEY: &str = "hot";

fn static_key(i: usize) -> String {
    format!("static_{i:02}")
}

fn static_value(i: usize) -> Vec<u8> {
    common::bytes(8, i as u8)
}

// ---------------------------------------------------------------------------
// Child process
// ---------------------------------------------------------------------------

/// The child body. `#[ignore]` keeps it out of a normal run, and the missing environment
/// variable makes it a no-op, so `cargo test -- --ignored` stays green too.
#[test]
#[ignore = "re-executed as a child process by the crash tests in this file"]
fn crash_child() {
    let Ok(dir) = std::env::var(DIR_VAR) else {
        return;
    };
    let mmkv = common::open_dir(&dir).expect("child could not open the store");

    // A fresh store is exactly one page long, and the page size is whatever the platform
    // reports (4 KiB on x86-64 Linux, 16 KiB on arm64 macOS), so measure it rather than
    // assume it. Half a page per value means two copies of the hot record never fit and
    // every rewrite below has to go through the shadow-file trim.
    let page = fs::metadata(Path::new(&dir).join(DATA_FILE_NAME))
        .expect("child could not stat the data file")
        .len();
    let hot_len = (page / 2) as usize;

    for i in 0..STATIC_KEYS {
        mmkv.put(&static_key(i), static_value(i).as_slice())
            .expect("child could not write a static key");
    }

    // `put` returns only once the record is in the file, so every static key above is now
    // durable as far as this process is concerned. Tell the parent it may start killing.
    fs::write(Path::new(&dir).join(READY_FILE), b"1").expect("child could not signal ready");

    let mut round = 0u32;
    loop {
        let value = common::bytes(hot_len, (round % 251) as u8);
        mmkv.put(HOT_KEY, value.as_slice())
            .expect("child could not rewrite the hot key");
        round = round.wrapping_add(1);
    }
}

// ---------------------------------------------------------------------------
// Parent helpers
// ---------------------------------------------------------------------------

/// Run [`crash_child`] against `store` and `SIGKILL` it `kill_after` past the moment it
/// reported readiness.
fn kill_child_while_it_trims(store: &Store, kill_after: Duration) {
    let exe = std::env::current_exe().expect("path of the running test binary");
    let mut child = Command::new(exe)
        .args(["--exact", "crash_child", "--ignored", "--test-threads=1"])
        .env(DIR_VAR, store.dir())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("could not spawn the crash child");

    wait_until_ready(store, &mut child);
    std::thread::sleep(kill_after);

    child.kill().expect("could not kill the crash child");
    let status = child.wait().expect("could not reap the crash child");
    assert!(
        !status.success(),
        "the child was supposed to die by SIGKILL, it exited with {status:?}"
    );
}

/// Block until the child has written every static key, or fail loudly.
fn wait_until_ready(store: &Store, child: &mut Child) {
    let marker = store.dir().join(READY_FILE);
    let deadline = Instant::now() + Duration::from_secs(60);
    while Instant::now() < deadline {
        if marker.exists() {
            return;
        }
        if let Some(status) = child.try_wait().expect("could not poll the crash child") {
            panic!("the child exited with {status:?} before it was ready");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = child.kill();
    panic!("the child never reported readiness");
}

/// Reopen the killed store and check it against what the child had acknowledged.
///
/// `page` is the length of a freshly created store. The child rewrites half a page at a
/// time forever, so a file still that length is proof the writer really was trimming
/// when it died rather than quietly appending — without it these tests could pass on a
/// store that never reached the code path they exist to cover.
fn assert_store_recovered(store: &Store, page: u64) {
    assert_eq!(
        store.file_len(),
        page,
        "the child should have been trimming, but the file grew"
    );

    let mmkv = store
        .try_open()
        .expect("the store must reopen after the crash");

    for i in 0..STATIC_KEYS {
        assert_eq!(
            mmkv.get::<Vec<u8>>(&static_key(i)),
            Ok(static_value(i)),
            "{} was acknowledged before the kill and must survive it",
            static_key(i)
        );
    }

    // The hot key was being rewritten when the kill landed, so it may be missing
    // entirely. What it must never be is a value the child never wrote: every value it
    // wrote was one byte repeated `hot_len` times.
    match mmkv.get::<Vec<u8>>(HOT_KEY) {
        Ok(value) => {
            assert!(!value.is_empty(), "{HOT_KEY} decoded to an empty value");
            assert!(
                value.iter().all(|byte| *byte == value[0]),
                "{HOT_KEY} decoded to a value the child never wrote"
            );
        }
        Err(KeyNotFound) => {}
        Err(e) => panic!("{HOT_KEY} must decode or be absent, got {e:?}"),
    }

    // A trim killed before its rename leaves its shadow file behind; opening the store
    // sweeps it. Nothing but the data file and (under encryption) the meta file is left.
    assert_no_shadow_files(store);

    // And the recovered store is still a working store, not just a readable one.
    mmkv.put("written_after_the_crash", b"ok".as_slice())
        .expect("the recovered store must still accept writes");
    assert_eq!(
        mmkv.get::<Vec<u8>>("written_after_the_crash"),
        Ok(b"ok".to_vec())
    );
}

fn assert_no_shadow_files(store: &Store) {
    let prefix = format!("{DATA_FILE_NAME}.tmp.");
    let leftovers: Vec<_> = fs::read_dir(store.dir())
        .expect("could not list the store directory")
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(&prefix))
        .collect();
    assert!(
        leftovers.is_empty(),
        "shadow files left behind after reopening: {leftovers:?}"
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Kill the writer at a spread of points inside its rewrite loop. Each delay lands the
/// `SIGKILL` somewhere different relative to the trim's commit points — building the
/// shadow file, syncing it, (under encryption) persisting the new nonce to the meta
/// file, renaming it over the live file — and every one of them must reopen with all the
/// acknowledged keys intact.
#[test]
fn killing_the_writer_mid_trim_keeps_every_acknowledged_key() {
    for delay_ms in [15, 40, 90, 200] {
        let store = Store::new();
        let page = new_store_page(&store);
        kill_child_while_it_trims(&store, Duration::from_millis(delay_ms));
        assert_store_recovered(&store, page);
    }
}

/// The length of a store that holds nothing, which is one page of whatever size the
/// platform uses. Creating it here also means the child reopens an existing store rather
/// than racing to create one.
fn new_store_page(store: &Store) -> u64 {
    drop(store.open());
    store.file_len()
}

/// Killing the writer between two whole `put` calls is the easy case, and it pins the
/// documented contract: `put` returns once the record is in the file, so a process death
/// right afterwards cannot lose it.
#[test]
fn a_process_killed_after_a_put_still_has_the_value() {
    let store = Store::new();
    // No delay: the kill lands as soon as the static keys are acknowledged, before the
    // rewrite loop has had a chance to trim anything.
    kill_child_while_it_trims(&store, Duration::ZERO);

    let mmkv = store.try_open().expect("the store must reopen");
    for i in 0..STATIC_KEYS {
        assert_eq!(mmkv.get::<Vec<u8>>(&static_key(i)), Ok(static_value(i)));
    }
}

/// A trim that died before its rename leaves a complete shadow file next to an untouched
/// live file. The live file is the committed state, so the next open must ignore the
/// shadow copy and remove it instead of adopting it.
#[test]
fn an_orphan_shadow_file_from_a_crashed_trim_is_swept() {
    let store = Store::new();
    let mmkv = store.open();
    mmkv.put("k1", b"v1".as_slice()).unwrap();
    mmkv.put("k2", b"v2".as_slice()).unwrap();
    drop(mmkv);

    let orphan = store.dir().join(format!("{DATA_FILE_NAME}.tmp.7"));
    fs::write(&orphan, b"a half written shadow file").unwrap();

    let mmkv = store.open();
    assert_eq!(mmkv.get::<Vec<u8>>("k1"), Ok(b"v1".to_vec()));
    assert_eq!(mmkv.get::<Vec<u8>>("k2"), Ok(b"v2".to_vec()));
    assert!(
        !orphan.exists(),
        "opening the store must sweep the orphaned shadow file"
    );
    assert_no_shadow_files(&store);
}
