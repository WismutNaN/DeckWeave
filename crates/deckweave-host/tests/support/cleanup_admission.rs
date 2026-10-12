// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
struct RejectedCleanup;
impl RuntimeIntentDispatcher for RejectedCleanup {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure> {
        if matches!(
            command,
            RuntimeCommand::CleanupAll { .. } | RuntimeCommand::CleanupRun { .. }
        ) {
            return Err(RuntimeDispatchFailure {
                error: test_error(),
                possibly_sent: false,
            });
        }
        Ok(())
    }
}
#[test]
fn cleanup_admission_failure_is_returned_by_barrier() -> Result<(), Box<dyn Error>> {
    let owner = LiveExecutionOwner::new(
        HostBootId::try_new("cleanup-admission")?,
        Arc::new(RejectedCleanup),
    );
    let (graph, bound, source) = fixture()?;
    let epoch = owner.activate("plan", Some(&graph), Some(&bound), GraphLimits::default())?;
    owner.publish(signal(epoch, source, true));
    assert!(matches!(
        owner.close_barrier(1),
        Err(deckweave_host::LiveExecutionError::Cleanup(_))
    ));
    Ok(())
}
#[test]
fn old_runtime_identity_is_rejected_even_with_the_current_epoch_and_stable_endpoint()
-> Result<(), Box<dyn Error>> {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let owner = LiveExecutionOwner::new(HostBootId::try_new("source-fence")?, dispatcher.clone());
    let (graph, bound, source) = fixture()?;
    let epoch = owner.activate("plan", Some(&graph), Some(&bound), GraphLimits::default())?;
    let mut stale = source.clone();
    stale.session_generation = SessionGeneration::try_new(source.session_generation.get() + 1)?;
    owner.publish(signal(epoch, stale, true));
    assert_eq!(feedback_count(&dispatcher), 0);
    owner.publish(signal(epoch, source, true));
    assert_eq!(feedback_count(&dispatcher), 1);
    Ok(())
}
