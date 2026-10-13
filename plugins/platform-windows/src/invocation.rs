// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    PlatformFailure,
    audio::EffectKnowledge,
    control_audio::{ControlAudio, parse_invocation, state_payload},
    control_window::{
        ControlWindow, WindowResult, parse_invocation as parse_window_invocation,
        result_payload as window_result_payload,
    },
    wire::WireSender,
};
use deckweave_protocol::v1::{
    InvocationReply, TerminalOutcome, invocation_reply, protocol_envelope, terminal_outcome,
};
use std::error::Error;
pub(crate) fn invoke(
    wire: &WireSender,
    audio: Option<&ControlAudio>,
    window: Option<&ControlWindow>,
    request: deckweave_protocol::v1::InvocationRequest,
) -> Result<(), Box<dyn Error>> {
    let invocation_id = request.invocation_id.clone();
    let reply = match parse_invocation(&request) {
        Ok(Some(invocation)) => match audio
            .ok_or(PlatformFailure::OsApiUnavailable)?
            .apply(invocation)
        {
            Ok(state) => invocation_reply::Reply::Terminal(TerminalOutcome {
                invocation_id: invocation_id.clone(),
                result: Some(terminal_outcome::Result::Succeeded(state_payload(&state)?)),
            }),
            Err(error) if error.effect == EffectKnowledge::PossiblyApplied => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id: invocation_id.clone(),
                    result: Some(terminal_outcome::Result::UnknownOutcome(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) => {
                invocation_reply::Reply::Rejected(PlatformFailure::from(error.failure).structured())
            }
        },
        Ok(None) => match crate::media::invoke(&request) {
            Ok(Some(reply)) => reply,
            Ok(None) => window_reply(window, &request)?,
            Err(error) => invocation_reply::Reply::Rejected(error.structured()),
        },
        Err(error) => invocation_reply::Reply::Rejected(error.structured()),
    };
    wire.send(protocol_envelope::Body::InvocationReply(InvocationReply {
        invocation_id,
        reply: Some(reply),
    }))?;
    Ok(())
}

fn window_reply(
    window: Option<&ControlWindow>,
    request: &deckweave_protocol::v1::InvocationRequest,
) -> Result<invocation_reply::Reply, Box<dyn Error>> {
    let invocation_id = request.invocation_id.clone();
    let reply = match parse_window_invocation(request) {
        Ok(Some(invocation)) => match window
            .ok_or(PlatformFailure::OsApiUnavailable)?
            .apply(invocation)
        {
            Ok(result @ (WindowResult::State(_) | WindowResult::Catalog(_))) => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::Succeeded(window_result_payload(
                        &result,
                    )?)),
                })
            }
            Err(error)
                if matches!(
                    error.failure,
                    crate::window::WindowFailure::ForegroundDenied
                        | crate::window::WindowFailure::StaleReference
                ) =>
            {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::Failed(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) if error.effect == crate::window::EffectKnowledge::PossiblyApplied => {
                invocation_reply::Reply::Terminal(TerminalOutcome {
                    invocation_id,
                    result: Some(terminal_outcome::Result::UnknownOutcome(
                        PlatformFailure::from(error.failure).structured(),
                    )),
                })
            }
            Err(error) => {
                invocation_reply::Reply::Rejected(PlatformFailure::from(error.failure).structured())
            }
        },
        Ok(None) => {
            invocation_reply::Reply::Rejected(PlatformFailure::controlled_adapter().structured())
        }
        Err(error) => invocation_reply::Reply::Rejected(error.structured()),
    };
    Ok(reply)
}
