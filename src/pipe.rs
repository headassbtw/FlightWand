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
    /// Updates the backend's knowledge of "up"
    UpdateIdentity([f32; 3]),
}

pub struct VRSystemInformation {
    pub system_properties: SystemProperties,
}

pub struct VRInputBounds {
    /// Minimum value to be outputted to the stick
    pub stick_deadzone: f32,
    /// Remaps `0..=stick_max` to `0.0..=1.0` for gamepad output
    pub stick_max: f32,
}

impl Default for VRInputBounds {
    fn default() -> Self { Self { stick_deadzone: 0.1, stick_max: 0.85 } }
}

pub enum VRSystemFailure {
    /// Couldn't initialize the virtual gamepad.
    // TODO: add the error that evdev throws
    VirtualGamepad(std::io::Error),
    /// Couldn't start OpenXR.
    // TODO: add the error that OpenXR throws
    EntryCreation(openxr::LoadError),
    /// OpenXR couldn't finish starting up.
    Generic(openxr::sys::Result),
    /// Vulkan creation error.
    Vulkan(ash::vk::Result),
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
            VRSystemFailure::VirtualGamepad(err) => write!(f, "Virtual gamepad error.\n{:?}", err),
            VRSystemFailure::EntryCreation(err) => {
                write!(f, "Failed to create OpenXR entry.\n\nAdvanced: {:?}", err)
            }
            VRSystemFailure::RotationUnavailable => {
                write!(f, "The selected VR system does not support rotational tracking.")
            }
            VRSystemFailure::Generic(res) => write!(f, "OpenXR failure:\n{}\nXrResult::{:?}", res, res),
            VRSystemFailure::Vulkan(res) => write!(f, "Vulkan failure:\n{:?}", res),
            VRSystemFailure::VulkanMismatch => write!(f, "Your system does not support Vulkan 1.1."),
            VRSystemFailure::VulkanUnavailable => write!(f, "The system does not have a usable Vulkan implementation."),
            VRSystemFailure::VulkanLoader(err) => write!(f, "Vulkan loader failure: {:?}", err),
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
