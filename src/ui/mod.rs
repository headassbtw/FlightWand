mod graph;
mod graph3d;

use crate::{
    pipe::{self, UI2VR, VR2UI, VRSystemFailure},
    ui::graph3d::Graph3D,
    util,
};
use eframe::emath::Align;
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, Layout, TextStyle, Widget, vec2,
    widgets::{DragValue, Slider},
};
use log::info;
use openxr::SystemProperties;

pub struct UI {
    tx: std::sync::mpsc::Sender<UI2VR>,
    rx: std::sync::mpsc::Receiver<VR2UI>,
    startup_hand: pipe::Hand,
    system_properties: Option<SystemProperties>,
    startup_failure: Option<VRSystemFailure>,
    runtime_failure: Option<VRSystemFailure>,
    graph3d: Graph3D,
    graph: [[f32; 4]; 100],
    id_mod: [f32; 3],
    offsets: [f32; 3],
}

impl UI {
    pub fn new(
        tx: std::sync::mpsc::Sender<UI2VR>,
        rx: std::sync::mpsc::Receiver<VR2UI>,
        cc: &eframe::CreationContext,
    ) -> Self {
        let mut id_mod = [0.0; 3];
        let mut offsets = [0.0; 3];
        id_mod[2] = -1.0;
        offsets[0] = -0.18;
        offsets[2] = -0.35;

        cc.egui_ctx.style_mut(|style| {
            for (style, font) in &mut style.text_styles {
                match style {
                    TextStyle::Body => font.size = 19.0,
                    TextStyle::Heading => font.size = 36.0,
                    _ => {}
                }
            }
        });

        let mut fonts = FontDefinitions::default();

        fonts.font_data.insert("aldrich".to_owned(), FontData::from_static(include_bytes!("../../fonts/Aldrich.ttf")));

        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "aldrich".to_owned());

        cc.egui_ctx.set_fonts(fonts);

        Self {
            tx,
            rx,
            startup_hand: pipe::Hand::Right,
            system_properties: None,
            startup_failure: None,
            runtime_failure: None,
            graph: [[0.0; 4]; 100],
            offsets,
            id_mod,
            graph3d: Graph3D::new(cc),
        }
    }

    pub fn run(tx: std::sync::mpsc::Sender<UI2VR>, rx: std::sync::mpsc::Receiver<VR2UI>) -> eframe::Result<()> {
        let rtn =
            eframe::run_native("FlightWand", Default::default(), Box::new(|cc| Ok(Box::new(UI::new(tx, rx, cc)))));
        info!("Frontend stopped");
        rtn
    }
}

impl eframe::App for UI {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                VR2UI::Running(inf) => {
                    self.system_properties = Some(inf.system_properties);
                }
                VR2UI::Failure(inf) => {
                    if self.startup_failure.is_none() {
                        self.startup_failure = Some(inf);
                    } else {
                        self.runtime_failure = Some(inf);
                    }
                }
                VR2UI::RotationUpdate(quat) => {
                    let mut i = 0;
                    while i < 99 {
                        self.graph[i] = self.graph[i + 1];
                        i += 1;
                    }
                    self.graph[99][0] = quat.x;
                    self.graph[99][1] = quat.y;
                    self.graph[99][2] = quat.z;
                    self.graph[99][3] = quat.w;
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(failure) = &self.startup_failure {
                ui.scope(|ui| {
                    ui.heading(egui::RichText::from("Startup Failed").color(Color32::RED));

                    ui.separator();

                    ui.label(format!("{}", failure));
                });

                return;
            }

            match &self.system_properties {
                Some(inf) => {
                    ui.heading(format!("{}", &inf.system_name));
                    ui.separator();
                }
                None => {
                    ui.heading("Not Running.");

                    let rect = egui::Rect::from_center_size(
                        ui.available_rect_before_wrap().center(),
                        vec2(200.0, 50.0 + ui.spacing().item_spacing.y + ui.spacing().interact_size.y),
                    );
                    let cursor = vec2(
                        (ui.available_rect_before_wrap().width() - rect.width()) / 2.0 - ui.spacing().item_spacing.x,
                        (ui.available_rect_before_wrap().height() - rect.height()) / 2.0 - ui.spacing().item_spacing.y,
                    );

                    ui.horizontal(|ui| {
                        ui.allocate_space(cursor);
                        ui.vertical(|ui| {
                            ui.allocate_space(cursor);

                            egui::ComboBox::from_id_salt("HandComboBox")
                                .width(200.0)
                                .height(50.0)
                                .selected_text(format!("{} Hand", self.startup_hand))
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(&mut self.startup_hand, pipe::Hand::Left, "Left Hand");
                                    ui.selectable_value(&mut self.startup_hand, pipe::Hand::Right, "Right Hand");
                                });

                            if ui.add_sized(vec2(200.0, 50.0), egui::Button::new("Start")).clicked() {
                                let _ = self.tx.send(UI2VR::Start(self.startup_hand.clone()));
                            }
                        });
                    });

                    return;
                }
            }

            ui.label("Offset/Centering: ");
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::RED;
                let x_changed = Slider::new(&mut self.offsets[0], -1.0..=1.0).show_value(false).ui(ui).changed();
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::GREEN;
                let y_changed = Slider::new(&mut self.offsets[1], -1.0..=1.0).show_value(false).ui(ui).changed();
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::BLUE;
                let z_changed = Slider::new(&mut self.offsets[2], -1.0..=1.0).show_value(false).ui(ui).changed();

                if x_changed || y_changed || z_changed {
                    let _ = self.tx.send(UI2VR::UpdateOffsets(self.offsets));
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.x = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::RED;
                let x_changed = DragValue::new(&mut self.offsets[0]).range(-1.0..=1.0).ui(ui).changed();
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::GREEN;
                let y_changed = DragValue::new(&mut self.offsets[1]).range(-1.0..=1.0).ui(ui).changed();
                ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::BLUE;
                let z_changed = DragValue::new(&mut self.offsets[2]).range(-1.0..=1.0).ui(ui).changed();

                if x_changed || y_changed || z_changed {
                    let _ = self.tx.send(UI2VR::UpdateOffsets(self.offsets));
                }
            });

            let mut buffer: [[f32; 4]; 100] = [[0.0; 4]; 100];
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let mut i = 0;
                while i < self.graph.len() {
                    buffer[i] = util::modifier(&self.graph[i]);
                    i += 1;
                }
                self.graph3d.draw(&buffer, ui);
                graph::graph(&self.graph, ui, util::modifier);
            });
        });
    }

    fn on_exit(&mut self, _ctx: Option<&eframe::glow::Context>) { let _ = self.tx.send(UI2VR::Shutdown); }
}
