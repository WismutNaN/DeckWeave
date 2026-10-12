// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{sync::mpsc, time::Duration};

struct PausedAdmission {
    entered: mpsc::SyncSender<()>,
    release: Mutex<mpsc::Receiver<()>>,
    commands: Mutex<Vec<RuntimeCommand>>,
}
impl RuntimeIntentDispatcher for PausedAdmission {
    fn dispatch(&self, command: RuntimeCommand) -> Result<(), RuntimeDispatchFailure> {
        // Test-only pause at the exact admission boundary; real dispatch never blocks.
        if matches!(command, RuntimeCommand::Feedback { .. }) {
            let _ = self.entered.try_send(());
            if let Ok(receiver) = self.release.lock() {
                let _ = receiver.recv_timeout(Duration::from_secs(2));
            }
        }
        if let Ok(mut commands) = self.commands.lock() {
            commands.push(command);
        }
        Ok(())
    }
}

#[test]
fn barrier_cannot_overtake_admission_from_an_earlier_transition() -> Result<(), Box<dyn Error>> {
    let (entered, ready) = mpsc::sync_channel(1);
    let (release, receive) = mpsc::sync_channel(1);
    let dispatcher = Arc::new(PausedAdmission {
        entered,
        release: Mutex::new(receive),
        commands: Mutex::new(vec![]),
    });
    let owner = Arc::new(LiveExecutionOwner::new(
        HostBootId::try_new("ordering")?,
        dispatcher.clone(),
    ));
    let (graph, binding, source) = fixture()?;
    let epoch = owner.activate(
        "ordered",
        Some(&graph),
        Some(&binding),
        GraphLimits::default(),
    )?;
    let ingress = owner.clone();
    let input_thread = std::thread::spawn(move || ingress.publish(signal(epoch, source, true)));
    ready.recv_timeout(Duration::from_secs(2))?;
    let barrier_owner = owner.clone();
    let (done, completed) = mpsc::sync_channel(1);
    let barrier_thread = std::thread::spawn(move || {
        let result = barrier_owner.close_barrier(1);
        let _ = done.send(result);
    });
    let overtook = completed.recv_timeout(Duration::from_millis(50)).is_ok();
    release.send(())?;
    input_thread.join().map_err(|_| "input thread")?;
    barrier_thread.join().map_err(|_| "barrier thread")?;
    assert!(!overtook);
    completed.recv_timeout(Duration::from_secs(2))??;
    let commands = dispatcher.commands.lock().map_err(|_| "commands")?;
    assert!(matches!(
        commands.first(),
        Some(RuntimeCommand::Feedback { .. })
    ));
    assert!(matches!(
        commands.last(),
        Some(RuntimeCommand::CleanupAll { .. })
    ));
    Ok(())
}

#[test]
fn failed_activation_preserves_the_previous_scheduler_and_epoch() -> Result<(), Box<dyn Error>> {
    let dispatcher = Arc::new(RecordingDispatcher::default());
    let owner = LiveExecutionOwner::new(HostBootId::try_new("activation")?, dispatcher.clone());
    let (graph, binding, source) = fixture()?;
    let epoch = owner.activate(
        "valid",
        Some(&graph),
        Some(&binding),
        GraphLimits::default(),
    )?;
    assert!(
        owner
            .activate("invalid", Some(&graph), None, GraphLimits::default())
            .is_err()
    );
    assert_eq!(owner.activation_epoch()?, Some(epoch));
    owner.publish(signal(epoch, source, true));
    assert_eq!(feedback_count(&dispatcher), 1);
    Ok(())
}
