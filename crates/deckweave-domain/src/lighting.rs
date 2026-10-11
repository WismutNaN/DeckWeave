// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{
    ids::{DescriptorRevision, ElementId, LightGroupId, LightUnitId, NamespacedKind},
    schema::ExtensionMap,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryUnit {
    Millimeter,
    Normalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeometryConfidence {
    Measured,
    VendorDocumented,
    Estimated,
    UserCalibrated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point3 {
    pub x: f64,
    pub y: f64,
    pub z: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoordinateFrame {
    pub id: NamespacedKind,
    pub unit: GeometryUnit,
    pub origin: Point3,
    pub x_axis: Point3,
    pub y_axis: Point3,
    pub bounds_min: Point3,
    pub bounds_max: Point3,
    pub confidence: GeometryConfidence,
    pub evidence: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightUnitDescriptor {
    pub light_unit_id: LightUnitId,
    pub position: Option<Point3>,
    #[serde(default)]
    pub groups: Vec<LightGroupId>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightGroupDescriptor {
    pub light_group_id: LightGroupId,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElementLightMap {
    pub element_id: ElementId,
    pub light_unit_ids: Vec<LightUnitId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LightTopology {
    pub topology_revision: DescriptorRevision,
    pub coordinate_frame: CoordinateFrame,
    #[serde(default)]
    pub light_units: Vec<LightUnitDescriptor>,
    #[serde(default)]
    pub groups: Vec<LightGroupDescriptor>,
    #[serde(default)]
    pub element_mappings: Vec<ElementLightMap>,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LightTopologyError {
    #[error("повторяющийся light unit или group: {0}")]
    DuplicateId(String),
    #[error("mapping или unit ссылается на неизвестный light unit/group: {0}")]
    UnknownReference(String),
    #[error("bounds geometry заданы в обратном порядке")]
    InvalidBounds,
}

impl LightTopology {
    pub fn validate(&self) -> Result<(), LightTopologyError> {
        let frame = &self.coordinate_frame;
        if frame.bounds_min.x > frame.bounds_max.x
            || frame.bounds_min.y > frame.bounds_max.y
            || frame
                .bounds_min
                .z
                .zip(frame.bounds_max.z)
                .is_some_and(|(min, max)| min > max)
        {
            return Err(LightTopologyError::InvalidBounds);
        }
        let mut units = BTreeSet::new();
        for unit in &self.light_units {
            if !units.insert(unit.light_unit_id.as_str()) {
                return Err(LightTopologyError::DuplicateId(
                    unit.light_unit_id.to_string(),
                ));
            }
        }
        let mut groups = BTreeSet::new();
        for group in &self.groups {
            if !groups.insert(group.light_group_id.as_str()) {
                return Err(LightTopologyError::DuplicateId(
                    group.light_group_id.to_string(),
                ));
            }
        }
        for unit in &self.light_units {
            for group in &unit.groups {
                if !groups.contains(group.as_str()) {
                    return Err(LightTopologyError::UnknownReference(group.to_string()));
                }
            }
        }
        for mapping in &self.element_mappings {
            for unit in &mapping.light_unit_ids {
                if !units.contains(unit.as_str()) {
                    return Err(LightTopologyError::UnknownReference(unit.to_string()));
                }
            }
        }
        Ok(())
    }
}
