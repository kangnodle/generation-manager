use gm_application::RunStateRepository;
use gm_core::generation::{Generation,
                          GenerationId,
                          GenerationStatus};
use gm_store::{Layout,
               ProjectLock,
               Store};
use std::{path::PathBuf,
          sync::atomic::{AtomicU32,
                         Ordering}};

static COUNTER: AtomicU32 = AtomicU32::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> TempProject {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("gm-test-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        match std::fs::create_dir_all(&path) {
            | Ok(()) => (),
            | Err(error) => panic!("failed to create temporary project {path:?}: {error}"),
        };
        TempProject(path)
    }

    fn open_store(&self) -> Store {
        match Store::open(Layout::new(&self.0)) {
            | Ok(store) => store,
            | Err(error) => panic!("failed to open test store at {:?}: {error}", self.0),
        }
    }
}

impl Drop for TempProject {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn lock_store(store: &Store) -> ProjectLock {
    match store.lock() {
        | Ok(lock) => lock,
        | Err(error) => panic!("failed to lock test store: {error}"),
    }
}

fn switch_generation(store: &Store, lock: &ProjectLock, id: GenerationId) {
    match store.switch(lock, id, "test") {
        | Ok(()) => (),
        | Err(error) => panic!("failed to switch to generation {id}: {error}"),
    }
}

fn current_id(store: &Store) -> Option<GenerationId> {
    match store.current_id() {
        | Ok(id) => id,
        | Err(error) => panic!("failed to read current generation: {error}"),
    }
}

fn generation(store: &Store, id: GenerationId) -> gm_store::GenerationEntry {
    match store.get(id) {
        | Ok(entry) => entry,
        | Err(error) => panic!("failed to load generation {id}: {error}"),
    }
}

fn generations(store: &Store) -> Vec<gm_store::GenerationEntry> {
    match store.list() {
        | Ok(entries) => entries,
        | Err(error) => panic!("failed to list generations: {error}"),
    }
}

fn rollback_target(store: &Store) -> GenerationId {
    match store.rollback_target() {
        | Ok(id) => id,
        | Err(error) => panic!("failed to select rollback target: {error}"),
    }
}

fn read_contents(path: PathBuf) -> String {
    match std::fs::read_to_string(&path) {
        | Ok(contents) => contents,
        | Err(error) => panic!("failed to read {path:?}: {error}"),
    }
}

fn add_generation(store: &Store, lock: &ProjectLock, commit: &str) -> GenerationId {
    let staged = match store.stage(lock, Some(commit)) {
        | Ok(staged) => staged,
        | Err(error) => panic!("failed to stage generation for {commit}: {error}"),
    };
    match std::fs::write(staged.payload.join("marker"), commit) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to write marker for {commit}: {error}"),
    };
    let id = staged.id;
    let meta = Generation {
        id,
        commit: Some(commit.to_string()),
        worktree: None,
        dirty: false,
        built_at: std::time::SystemTime::now(),
        status: GenerationStatus::Built,
        artifacts: vec![PathBuf::from("marker")],
        note: None,
    };
    match store.commit(lock, staged, meta) {
        | Ok(_) => (),
        | Err(error) => panic!("failed to commit generation {id}: {error}"),
    };
    id
}

#[test]
fn generations_are_numbered_from_one() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);

    assert_eq!(add_generation(&store, &lock, "aaaaaaa"), GenerationId(1));
    assert_eq!(add_generation(&store, &lock, "bbbbbbb"), GenerationId(2));
    assert_eq!(generations(&store).len(), 2);
}

#[test]
fn completed_run_cleanup_waits_for_the_lock_and_preserves_a_replacement() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let state = gm_store::FileRunState::new(store.layout().clone());
    let first = gm_core::RunState {
        source: gm_core::RunSource::Worktree {
            name: "first".into(),
        },
        pid: 10,
        process_start: Some("first-process".into()),
        started_at: std::time::SystemTime::now(),
        detached: false,
    };
    match state.write(&first) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to write first run state: {error}"),
    };
    let lock = lock_store(&store);
    let cleanup = state.clone();
    let completed = first.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::spawn(move || {
        match tx.send("started") {
            | Ok(()) => (),
            | Err(error) => panic!("failed to notify test that cleanup started: {error}"),
        };
        match cleanup.clear_if_matches(&completed) {
            | Ok(()) => (),
            | Err(error) => panic!("failed to clear completed run: {error}"),
        };
        match tx.send("finished") {
            | Ok(()) => (),
            | Err(error) => panic!("failed to notify test that cleanup finished: {error}"),
        };
    });
    match rx.recv() {
        | Ok(message) => assert_eq!(message, "started"),
        | Err(error) => panic!("failed to receive cleanup start: {error}"),
    }
    assert!(rx.recv_timeout(std::time::Duration::from_millis(30)).is_err());
    let second = gm_core::RunState {
        process_start: Some("replacement-process".into()),
        ..first.clone()
    };
    match state.write(&second) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to write second run state: {error}"),
    };
    drop(lock);
    match rx.recv_timeout(std::time::Duration::from_secs(2)) {
        | Ok(message) => assert_eq!(message, "finished"),
        | Err(error) => panic!("failed to receive cleanup completion: {error}"),
    }
    match thread.join() {
        | Ok(()) => (),
        | Err(panic) => std::panic::resume_unwind(panic),
    }
    match state.read() {
        | Some(run) => assert_eq!(run.process_start, second.process_start),
        | None => panic!("expected replacement run state to survive cleanup"),
    }
    match state.clear_if_matches(&second) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to clear replacement run state: {error}"),
    };
    assert!(state.read().is_none());
    match std::fs::create_dir(store.layout().run_state()) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to replace run state file with directory: {error}"),
    };
    assert!(state.write(&first).is_err());
}

#[test]
fn unreadable_generations_still_reserve_their_number() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    add_generation(&store, &lock, "aaaaaaa");
    let second = add_generation(&store, &lock, "bbbbbbb");
    let original = generation(&store, second).dir;
    match std::fs::write(Layout::generation_meta(&original), "invalid JSON") {
        | Ok(()) => (),
        | Err(error) => panic!("failed to corrupt generation metadata: {error}"),
    };

    assert_eq!(add_generation(&store, &lock, "bbbbbbb"), GenerationId(3));
    assert_eq!(read_contents(original.join("root/marker")), "bbbbbbb");
    assert_eq!(read_contents(Layout::generation_meta(&original)), "invalid JSON");
}

#[test]
fn commit_cannot_replace_a_published_generation() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    let id = add_generation(&store, &lock, "aaaaaaa");
    let entry = generation(&store, id);
    let mut meta = entry.meta.clone();
    meta.note = Some("replacement".into());
    let staged = gm_store::StagedGeneration {
        id,
        payload: entry.payload(),
        dir: entry.dir,
    };

    assert!(store.commit(&lock, staged, meta).is_err());
    assert_eq!(generation(&store, id).meta.note, None);
}

#[test]
fn nothing_is_active_before_the_first_switch() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    add_generation(&store, &lock, "aaaaaaa");

    assert_eq!(current_id(&store), None);
    assert!(store.current().is_err());
}

#[test]
fn switching_moves_the_current_pointer_and_records_history() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    let first = add_generation(&store, &lock, "aaaaaaa");
    let second = add_generation(&store, &lock, "bbbbbbb");

    switch_generation(&store, &lock, first);
    assert_eq!(current_id(&store), Some(first));

    switch_generation(&store, &lock, second);
    assert_eq!(current_id(&store), Some(second));
    let current = match store.current() {
        | Ok(entry) => entry,
        | Err(error) => panic!("failed to load current generation: {error}"),
    };
    assert_eq!(read_contents(current.payload().join("marker")), "bbbbbbb");

    let history = match store.history() {
        | Ok(history) => history,
        | Err(error) => panic!("failed to read switch history: {error}"),
    };
    assert_eq!(history.len(), 2);
    assert_eq!(history[1].from, Some(first));
    assert_eq!(history[1].to, second);
}

#[test]
fn rollback_target_is_the_highest_generation_below_current() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    let first = add_generation(&store, &lock, "aaaaaaa");
    let second = add_generation(&store, &lock, "bbbbbbb");
    let third = add_generation(&store, &lock, "ccccccc");

    switch_generation(&store, &lock, third);
    assert_eq!(rollback_target(&store), second);

    switch_generation(&store, &lock, second);
    assert_eq!(rollback_target(&store), first);

    switch_generation(&store, &lock, first);
    assert!(store.rollback_target().is_err());
}

#[test]
fn switching_to_a_missing_generation_leaves_current_untouched() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    let first = add_generation(&store, &lock, "aaaaaaa");
    switch_generation(&store, &lock, first);

    assert!(store.switch(&lock, GenerationId(99), "test").is_err());
    assert_eq!(current_id(&store), Some(first));
}

#[test]
fn gc_keeps_the_active_generation_and_its_rollback_target() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    let first = add_generation(&store, &lock, "aaaaaaa");
    let second = add_generation(&store, &lock, "bbbbbbb");
    let third = add_generation(&store, &lock, "ccccccc");
    let fourth = add_generation(&store, &lock, "ddddddd");

    switch_generation(&store, &lock, fourth);
    let removed = match store.gc(&lock, 2) {
        | Ok(removed) => removed,
        | Err(error) => panic!("failed to collect unused generations: {error}"),
    };

    assert_eq!(removed, vec![first, second]);
    let remaining: Vec<_> = generations(&store).into_iter().map(|e| e.meta.id).collect();
    assert_eq!(remaining, vec![third, fourth]);
    assert_eq!(current_id(&store), Some(fourth));
}

#[test]
fn a_second_lock_is_refused_while_the_first_is_held() {
    let temp = TempProject::new();
    let store = temp.open_store();

    let held = lock_store(&store);
    assert!(store.lock().is_err());
    drop(held);
    assert!(store.lock().is_ok());
}

#[test]
fn a_lock_from_another_project_cannot_mutate_the_store() {
    let first = TempProject::new();
    let second = TempProject::new();
    let store = first.open_store();
    let other = second.open_store();
    let wrong_lock = lock_store(&other);

    assert!(store.stage(&wrong_lock, Some("aaaaaaa")).is_err());
}

#[test]
fn latest_generation_skips_unreadable_entries_and_handles_an_empty_store() {
    let temp = TempProject::new();
    let store = temp.open_store();
    let lock = lock_store(&store);
    match store.latest() {
        | Ok(entry) => assert!(entry.is_none()),
        | Err(error) => panic!("failed to query latest generation in empty store: {error}"),
    }

    add_generation(&store, &lock, "aaaaaaa");
    let expected = add_generation(&store, &lock, "bbbbbbb");
    let corrupt = add_generation(&store, &lock, "ccccccc");
    let missing = add_generation(&store, &lock, "ddddddd");
    let corrupt_entry = generation(&store, corrupt);
    match std::fs::write(Layout::generation_meta(&corrupt_entry.dir), "invalid JSON") {
        | Ok(()) => (),
        | Err(error) => panic!("failed to corrupt generation metadata: {error}"),
    };
    let missing_entry = generation(&store, missing);
    match std::fs::remove_dir_all(missing_entry.dir) {
        | Ok(()) => (),
        | Err(error) => panic!("failed to remove generation {missing}: {error}"),
    };

    match store.latest() {
        | Ok(Some(entry)) => assert_eq!(entry.meta.id, expected),
        | Ok(None) => panic!("expected a readable generation"),
        | Err(error) => panic!("failed to load latest generation: {error}"),
    }
}
