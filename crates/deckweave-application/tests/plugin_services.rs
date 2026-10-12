// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_application::plugin_services::*;
use deckweave_domain::ids::*;
use std::{collections::BTreeSet, error::Error};

#[test]
fn delegation_preserves_owner_intersects_grants_and_caps_deadlines() -> Result<(), Box<dyn Error>> {
    let operation = NamespacedKind::try_new("org.fixture.publish")?;
    let owner = ServiceOwner {
        execution_id: ExecutionId::try_new("run")?,
        activation_epoch: ActivationEpoch::try_new(1)?,
        caller_instance: PluginInstanceId::try_new("hub")?,
        caller_generation: SessionGeneration::try_new(1)?,
        grant_revision: GrantRevision::try_new(1)?,
    };
    let parent = DelegationContext {
        owner: owner.clone(),
        deadline_host_ticks: 100,
        call_path: vec![owner.caller_instance.clone()],
        allowed_operations: BTreeSet::from([operation.clone()]),
    };
    let child = parent.child(
        PluginInstanceId::try_new("mqtt")?,
        &operation,
        &parent.allowed_operations,
        200,
        0,
    )?;
    assert_eq!(child.owner, parent.owner);
    assert_eq!(child.deadline_host_ticks, 100);
    assert_eq!(
        child.child(
            owner.caller_instance.clone(),
            &operation,
            &child.allowed_operations,
            100,
            0
        ),
        Err(ServiceCallError::Cycle)
    );
    assert_eq!(
        parent.child(
            PluginInstanceId::try_new("mqtt")?,
            &operation,
            &BTreeSet::new(),
            100,
            0
        ),
        Err(ServiceCallError::Denied)
    );
    assert_eq!(
        child.validate_fence(
            owner.activation_epoch,
            owner.caller_generation,
            GrantRevision::try_new(2)?
        ),
        Err(ServiceCallError::Denied)
    );
    assert_eq!(
        parent.child(
            PluginInstanceId::try_new("mqtt")?,
            &operation,
            &parent.allowed_operations,
            100,
            100
        ),
        Err(ServiceCallError::Deadline)
    );
    let mut cursor = SubscriptionCursor {
        provider_generation: SessionGeneration::try_new(1)?,
        revision: 5,
        sequence: 0,
    };
    assert_eq!(
        cursor.advance(SubscriptionCursor {
            sequence: 2,
            ..cursor.clone()
        }),
        Err(ServiceCallError::Stale)
    );
    cursor.advance(SubscriptionCursor {
        sequence: 1,
        ..cursor.clone()
    })?;
    Ok(())
}
