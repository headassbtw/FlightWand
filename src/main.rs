use crate::{
    pipe::{UI2VR, VR2UI},
    ui::UI,
    vrclient::VRClient,
};

pub mod pipe;
mod ui;
pub mod util;
mod vrclient;

#[allow(clippy::field_reassign_with_default)] // False positive, might be fixed 1.51
#[cfg_attr(target_os = "android", ndk_glue::main)]
#[tokio::main]
pub async fn main() -> eframe::Result {
    let (ui_tx, vr_rx) = std::sync::mpsc::channel::<UI2VR>();
    let (vr_tx, ui_rx) = std::sync::mpsc::channel::<VR2UI>();

    VRClient::run(vr_tx, vr_rx);
    UI::run(ui_tx, ui_rx)
}
