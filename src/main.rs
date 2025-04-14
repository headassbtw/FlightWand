use crate::{
    pipe::{UI2VR, VR2UI},
    ui::UI,
    vrclient::VRClient,
};

pub mod pipe;
mod ui;
pub mod util;
mod vrclient;

#[profiling::function]
pub fn main() -> eframe::Result {
    let (ui_tx, vr_rx) = std::sync::mpsc::channel::<UI2VR>();
    let (vr_tx, ui_rx) = std::sync::mpsc::channel::<VR2UI>();

    VRClient::run(vr_tx, vr_rx);
    UI::run(ui_tx, ui_rx)
}
