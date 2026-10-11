// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{ids::NamespacedKind, schema::ExtensionMap};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffineTransform2D {
    pub matrix: [f64; 6],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySurfaceDescriptor {
    pub width_pixels: u32,
    pub height_pixels: u32,
    pub rotation_degrees: u16,
    pub pixel_aspect_ratio: f64,
    pub coordinate_space: NamespacedKind,
    pub coordinate_transform: AffineTransform2D,
    #[serde(default)]
    pub pixel_formats: Vec<String>,
    pub color_space: String,
    pub alpha: bool,
    pub preferred_fps: u16,
    pub maximum_fps: u16,
    pub full_frame: bool,
    pub partial_update: bool,
    pub atlas: bool,
    pub persistent_frame: bool,
    #[serde(default, flatten)]
    pub extensions: ExtensionMap,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SurfaceDescriptorError {
    #[error("surface dimensions/FPS должны быть ненулевыми")]
    InvalidDimensions,
    #[error("preferred FPS выше maximum FPS")]
    InvalidFps,
    #[error("surface должна принимать хотя бы один pixel format")]
    MissingFormat,
}

impl DisplaySurfaceDescriptor {
    pub fn validate(&self) -> Result<(), SurfaceDescriptorError> {
        if self.width_pixels == 0
            || self.height_pixels == 0
            || self.preferred_fps == 0
            || self.maximum_fps == 0
            || self.pixel_aspect_ratio <= 0.0
        {
            return Err(SurfaceDescriptorError::InvalidDimensions);
        }
        if self.preferred_fps > self.maximum_fps {
            return Err(SurfaceDescriptorError::InvalidFps);
        }
        if self.pixel_formats.is_empty() {
            return Err(SurfaceDescriptorError::MissingFormat);
        }
        Ok(())
    }
}
