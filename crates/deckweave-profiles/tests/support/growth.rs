// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;

#[derive(Default)]
struct CountWrites(AtomicUsize);
impl AtomicFileStore for CountWrites {
    fn write_atomic(
        &self,
        target: &Path,
        bytes: &[u8],
        backup: Option<&Path>,
    ) -> Result<(), PublicationError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        NativeAtomicFileStore.write_atomic(target, bytes, backup)
    }
}

#[test]
fn long_editor_session_maintains_storage_across_restarts_with_constant_write_budget()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock::new(100));
    let files = Arc::new(CountWrites::default());
    let limits = ProfileStoreLimits {
        draft_checkpoint_interval: 16,
        dedup_window_seconds: 1,
        maximum_mutation_receipts: 64,
        ..Default::default()
    };
    let open = || {
        JsonProfileStore::with_mutation_clock(
            directory.path().into(),
            files.clone(),
            limits.clone(),
            clock.clone(),
        )
    };
    let mut store = open();
    let mut current = draft()?;
    store.create_draft(&current)?;
    let mut writes = Vec::new();
    for index in 0..256 {
        if index % 32 == 0 {
            clock.set(100 + index / 32);
        }
        if index % 64 == 0 {
            store = open();
        }
        let key = mutation_key(&store, &format!("growth-{index}"))?;
        let command = DraftCommand::Set {
            pointer: "/title".into(),
            value: serde_json::json!(index),
        };
        let before = files.0.load(Ordering::SeqCst);
        let saved =
            store.mutate_draft(&current.draft_id, key.clone(), current.version, &command)?;
        writes.push(files.0.load(Ordering::SeqCst) - before);
        assert!(
            store
                .mutate_draft(&current.draft_id, key, current.version, &command)?
                .replayed
        );
        current = saved.draft;
    }
    assert_eq!(open().load_draft(&current.draft_id)?.draft, current);
    assert!(writes.iter().all(|n| *n <= 6));
    assert!(
        writes[..128]
            .iter()
            .sum::<usize>()
            .abs_diff(writes[128..].iter().sum())
            <= 2
    );
    let draft_directory = fs::read_dir(directory.path().join("drafts"))?
        .next()
        .ok_or("draft dir")??;
    assert!(fs::read_dir(draft_directory.path().join("transactions"))?.count() <= 64);
    assert!(fs::read_dir(directory.path().join("mutations/receipts"))?.count() <= 32);
    Ok(())
}

#[test]
fn receipt_quota_collects_retired_epoch_even_before_scheduled_checkpoint()
-> Result<(), Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let clock = Arc::new(TestClock::new(100));
    let store = JsonProfileStore::with_mutation_clock(
        directory.path().into(),
        Arc::new(NativeAtomicFileStore),
        ProfileStoreLimits {
            maximum_mutation_receipts: 2,
            dedup_window_seconds: 1,
            ..Default::default()
        },
        clock.clone(),
    );
    let mut current = draft()?;
    store.create_draft(&current)?;
    for index in 0..4 {
        if index == 2 {
            clock.set(101);
        }
        current = store
            .mutate_draft(
                &current.draft_id,
                mutation_key(&store, &format!("quota-{index}"))?,
                current.version,
                &DraftCommand::Set {
                    pointer: "/title".into(),
                    value: serde_json::json!(index),
                },
            )?
            .draft;
    }
    assert_eq!(
        fs::read_dir(directory.path().join("mutations/receipts"))?.count(),
        2
    );
    Ok(())
}
