// SPDX-License-Identifier: GPL-3.0-or-later
use crate::RuntimeGroup;
use serde_json::{Value, json};
use std::error::Error;

const CONTROL_DESCRIPTOR: &[u8] = include_bytes!("../assets/control-descriptors.json");
const INPUT_DESCRIPTOR: &[u8] = include_bytes!("../assets/input-observe-descriptors.json");

pub(crate) fn runtime_descriptor(group: RuntimeGroup) -> Result<Vec<u8>, Box<dyn Error>> {
    if group == RuntimeGroup::InputObserve {
        return Ok(INPUT_DESCRIPTOR.to_vec());
    }
    let mut descriptor: Value = serde_json::from_slice(CONTROL_DESCRIPTOR)?;
    if cfg!(windows) {
        set_platform_availability(&mut descriptor)?;
    }
    Ok(serde_json::to_vec(&descriptor)?)
}

fn set_platform_availability(descriptor: &mut Value) -> Result<(), Box<dyn Error>> {
    let nodes = descriptor
        .as_array_mut()
        .ok_or("descriptor is not an array")?;
    for node in nodes {
        let Some(services) = node.get_mut("services").and_then(Value::as_array_mut) else {
            continue;
        };
        for service in services {
            let Some(elements) = service.get_mut("elements").and_then(Value::as_array_mut) else {
                continue;
            };
            for element in elements {
                let element_id = element
                    .get("elementId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                if !matches!(element_id.as_str(), "audio" | "applications") {
                    continue;
                }
                let capabilities = element
                    .get_mut("capabilities")
                    .and_then(Value::as_array_mut)
                    .ok_or("audio capabilities are absent")?;
                for capability in capabilities {
                    if element_id == "applications"
                        && capability.get("capabilityId").and_then(Value::as_str) == Some("launch")
                    {
                        continue;
                    }
                    let ports = capability
                        .get_mut("ports")
                        .and_then(Value::as_array_mut)
                        .ok_or("audio ports are absent")?;
                    for port in ports {
                        port["availability"] = json!({ "status": "available" });
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_implemented_windows_ports_become_available() -> Result<(), Box<dyn Error>> {
        let descriptor: Value =
            serde_json::from_slice(&runtime_descriptor(RuntimeGroup::Control)?)?;
        let mut ports = Vec::new();
        for element in descriptor[0]["services"][0]["elements"]
            .as_array()
            .ok_or("elements")?
        {
            for capability in element["capabilities"].as_array().ok_or("capabilities")? {
                for port in capability["ports"].as_array().ok_or("ports")? {
                    ports.push((element["elementId"].clone(), port.clone()));
                }
            }
        }
        for (element, port) in ports {
            let status = port["availability"]["status"].as_str().ok_or("status")?;
            let capability = port
                .get("tags")
                .and_then(Value::as_array)
                .is_some_and(|tags| tags.iter().any(|tag| tag == "window"));
            let implemented = element == "audio" || (element == "applications" && capability);
            if cfg!(windows) && implemented {
                assert_eq!(status, "available");
            } else {
                assert_eq!(status, "unavailable");
            }
        }
        Ok(())
    }
}
