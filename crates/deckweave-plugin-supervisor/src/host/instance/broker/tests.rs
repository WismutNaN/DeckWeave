// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use deckweave_domain::schema::{SchemaRef, SchemaVersion};
use deckweave_protocol::v1::{OpaquePayload, SchemaReference};

fn fixture() -> Result<(Tickets, HostProcessRequest), Box<dyn std::error::Error>> {
    let schema = "org.deckweave.windows.app_target";
    let input = serde_json::json!({"executable":"C:\\App\\app.exe","arguments":["literal & text"],"workingDirectory":null});
    let ticket = Ticket {
        epoch: 3,
        operation: "launch-process".into(),
        schema: SchemaRef {
            id: deckweave_domain::ids::NamespacedKind::try_new(schema)?,
            version: SchemaVersion { major: 1, minor: 0 },
        },
        input: input.clone(),
        deadline: 100,
        run_id: "run".into(),
    };
    let tickets = Tickets {
        pending: BTreeMap::from([("invocation".into(), ticket)]),
    };
    let request = HostProcessRequest {
        invocation_id: "invocation".into(),
        activation_epoch: 3,
        operation_id: "launch-process".into(),
        input: Some(OpaquePayload {
            schema: Some(SchemaReference {
                id: schema.into(),
                major: 1,
                minor: 0,
            }),
            value: serde_json::to_vec(&input)?,
        }),
    };
    Ok((tickets, request))
}
#[test]
fn identical_request_is_admitted_once_only() -> Result<(), Box<dyn std::error::Error>> {
    let (mut tickets, request) = fixture()?;
    assert!(tickets.claim(&request, 3, 1).is_ok());
    assert_eq!(
        tickets.claim(&request, 3, 1),
        Err(UserProcessFailure::Rejected)
    );
    Ok(())
}
#[test]
fn changed_payload_epoch_operation_or_expired_ticket_cannot_execute()
-> Result<(), Box<dyn std::error::Error>> {
    for case in 0..5 {
        let (mut tickets, mut request) = fixture()?;
        match case {
            0 => {
                request.input.as_mut().ok_or("input")?.value =
                    br#"{"executable":"C:\\other.exe","arguments":[]}"#.to_vec()
            }
            1 => request.activation_epoch = 4,
            2 => request.operation_id = "open-target".into(),
            3 => request.invocation_id = "other".into(),
            _ => {}
        }
        assert!(
            tickets
                .claim(&request, 3, if case == 4 { 100 } else { 1 })
                .is_err()
        );
    }
    Ok(())
}
