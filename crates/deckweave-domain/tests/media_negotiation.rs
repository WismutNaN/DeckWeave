// SPDX-License-Identifier: GPL-3.0-or-later
use deckweave_domain::media::*;
use serde_json::json;
use std::error::Error;

#[test]
fn media_negotiation_never_guesses_format_or_buffer_compatibility() -> Result<(), Box<dyn Error>> {
    let producer: StreamTemplate = serde_json::from_value(json!({
        "templateId":"org.fixture.audio", "direction":"output", "availability":"requestable",
        "formats":[{"mediaType":"audio/pcm","properties":{"rateHz":48000,"channels":2,"sample":"f32le"}}],
        "bufferBounds":{"minimumBytes":512,"maximumBytes":8192,"maximumInFlight":4},
        "maximumLatencyMs":20,"clockDomainRequired":true
    }))?;
    let mut consumer = producer.clone();
    consumer.direction = StreamDirection::Input;
    consumer.buffer_bounds.minimum_bytes = 1024;
    consumer.buffer_bounds.maximum_bytes = 4096;
    let agreed = producer.negotiate_with(&consumer)?;
    assert_eq!(agreed.buffer_bounds.minimum_bytes, 1024);
    assert_eq!(agreed.buffer_bounds.maximum_bytes, 4096);
    consumer.formats[0]
        .properties
        .insert("rateHz".into(), json!(44100));
    assert_eq!(
        producer.negotiate_with(&consumer),
        Err(StreamTemplateError::Incompatible)
    );
    consumer = producer.clone();
    consumer.direction = StreamDirection::Input;
    consumer.buffer_bounds.minimum_bytes = 16384;
    consumer.buffer_bounds.maximum_bytes = 32768;
    assert_eq!(
        producer.negotiate_with(&consumer),
        Err(StreamTemplateError::Incompatible)
    );
    Ok(())
}
