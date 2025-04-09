use ash::LoadingError;
use openxr::{Quaternionf, SystemProperties};
use std::fmt::Display;

#[derive(PartialEq, Copy, Clone)]
pub enum Hand {
    Left,
    Right,
}

impl Display for Hand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Hand::Left => write!(f, "Left"),
            Hand::Right => write!(f, "Right"),
        }
    }
}

pub enum UI2VR {
    /// Shuts the background down.
    Shutdown,
    /// Starts OpenXR.
    Start(Hand),
    /// Deprecated. Used to offset the raw value before I figured out quaternions.
    UpdateOffsets([f32; 3]),
    /// Updates the backend's knowledge of "up"
    UpdateIdentity([f32; 3]),
}

pub struct VRSystemInformation {
    pub system_properties: SystemProperties,
}

pub enum VRSystemFailure {
    /// Couldn't initialize the virtual gamepad.
    // TODO: add the error that evdev throws
    VirtualGamepad,
    /// Couldn't start OpenXR.
    // TODO: add the error that OpenXR throws
    EntryCreation,
    /// OpenXR couldn't finish starting up.
    Generic(openxr::sys::Result),
    /// Vulkan creation error.
    Vulkan(openxr::sys::platform::VkResult),
    /// The system does not have a usable Vulkan implementation.
    VulkanUnavailable,
    /// The system does not support Vulkan 1.1
    VulkanMismatch,
    /// Vulkan library loading error.
    VulkanLoader(LoadingError),
    /// EDGE CASE OF THE CENTURY.
    /// The attached XR system does not support rotational tracking.
    RotationUnavailable,
}

impl Display for VRSystemFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VRSystemFailure::VirtualGamepad => {
                write!(f, "Couldn't initialize the virtual gamepad.")
            }
            VRSystemFailure::EntryCreation => {
                write!(f, "Failed to create OpenXR entry.")
            }
            VRSystemFailure::RotationUnavailable => {
                write!(f, "The selected VR system does not support rotational tracking.")
            }
            VRSystemFailure::Generic(res) => match *res {
                openxr::sys::Result::ERROR_RUNTIME_UNAVAILABLE => {
                    write!(f, "OpenXR runtime unavailable.")
                }
                openxr::sys::Result::ERROR_RUNTIME_FAILURE => {
                    write!(f, "OpenXR runtime failure.")
                }
                _ => {
                    write!(f, "Other: {:?}", res)
                }
            },
            VRSystemFailure::Vulkan(res) => match *res {
                _ => {
                    write!(f, "Vulkan failure: {:?}", res)
                }
            },
            VRSystemFailure::VulkanMismatch => {
                write!(f, "Your system does not support Vulkan 1.1.")
            }
            VRSystemFailure::VulkanUnavailable => {
                write!(f, "Vulkan unavailable.")
            }
            VRSystemFailure::VulkanLoader(err) => {
                write!(f, "Vulkan loader failure: {:?}", err)
            }
        }
    }
}

pub enum VR2UI {
    /// Backend is running, show visualizations/settings/etc.
    Running(VRSystemInformation),
    /// Backend has failed. Application is no longer operational.
    Failure(VRSystemFailure),
    /// Controller rotation update (for visualization)
    RotationUpdate(Quaternionf),
}
