mod graph;
mod graph3d;

use crate::{
    pipe::{self, UI2VR, VR2UI, VRInputBounds, VRSystemFailure},
    ui::graph3d::Graph3D,
    util,
};
use eframe::{emath::Align, epaint::Stroke};
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, Id, LayerId, Layout, Order, Rounding, TextStyle, Widget,
    vec2,
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
    stick_bounds: VRInputBounds,
    graph: [[f32; 4]; 100],
    id_mod: [f32; 3],
}

#[profiling::all_functions]
impl UI {
    pub fn new(
        tx: std::sync::mpsc::Sender<UI2VR>,
        rx: std::sync::mpsc::Receiver<VR2UI>,
        cc: &eframe::CreationContext,
    ) -> Self {
        let id_mod = [0.0, 0.2, -1.0];

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
            id_mod,
            stick_bounds: VRInputBounds::default(),
            graph: [[0.0; 4]; 100],
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

#[profiling::all_functions]
impl UI {
    fn main_content(&mut self, ui: &mut egui::Ui) {
        ui.label("Up: ");
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::RED;
            let x_changed = Slider::new(&mut self.id_mod[0], -1.0..=1.0).show_value(false).ui(ui).changed();
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::GREEN;
            let y_changed = Slider::new(&mut self.id_mod[1], -1.0..=1.0).show_value(false).ui(ui).changed();
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::BLUE;
            let z_changed = Slider::new(&mut self.id_mod[2], -1.0..=1.0).show_value(false).ui(ui).changed();

            if x_changed || y_changed || z_changed {
                let _ = self.tx.send(UI2VR::UpdateIdentity(self.id_mod));
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().interact_size.x = (ui.available_width() - ui.spacing().item_spacing.x * 2.0) / 3.0;
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::RED;
            let x_changed = DragValue::new(&mut self.id_mod[0]).range(-1.0..=1.0).ui(ui).changed();
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::GREEN;
            let y_changed = DragValue::new(&mut self.id_mod[1]).range(-1.0..=1.0).ui(ui).changed();
            ui.style_mut().visuals.widgets.inactive.bg_fill = Color32::BLUE;
            let z_changed = DragValue::new(&mut self.id_mod[2]).range(-1.0..=1.0).ui(ui).changed();

            if x_changed || y_changed || z_changed {
                let _ = self.tx.send(UI2VR::UpdateIdentity(self.id_mod));
            }
        });

        ui.label("Current rotation: ");
        let mut buffer: [[f32; 4]; 100] = [[0.0; 4]; 100];
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            profiling::scope!("Rotation visualization");
            let mut i = 0;
            while i < self.graph.len() {
                profiling::scope!(&format!("data point {}", i));
                buffer[i] = util::modifier(&self.graph[i], self.id_mod);
                i += 1;
            }
            self.graph3d.draw(&buffer, ui);
            graph::graph(&buffer, self.id_mod, ui, |a, _| *a);
        });

        ui.label("Gamepad output: ");
        if ui
            .add_sized(
                vec2(ui.available_width(), ui.spacing().interact_size.y),
                Slider::new(&mut self.stick_bounds.deadzone, 0.0..=1.0).text("Deadzone"),
            )
            .changed()
            || ui
                .add_sized(
                    vec2(ui.available_width(), ui.spacing().interact_size.y),
                    Slider::new(&mut self.stick_bounds.stick_max, 0.0..=1.0).text("Maximum"),
                )
                .changed()
        {
            let _ = self.tx.send(UI2VR::UpdateBounds(self.stick_bounds.clone()));
        }
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            profiling::scope!("final joystick visualization");
            let mut i = 0;
            while i < self.graph.len() {
                profiling::scope!(&format!("data point {}", i));
                let tmp = util::modifier(&self.graph[i], self.id_mod);
                let [x, y] = util::rot_to_joy(&[tmp[0], tmp[2]], self.stick_bounds);

                buffer[i] = [x, y, -2.0, 2.0];
                i += 1;
            }

            let (rect, _) =
                ui.allocate_exact_size(egui::Vec2::splat(ui.spacing().interact_size.y * 10.0), egui::Sense::click());

            ui.painter().circle(
                rect.center(),
                rect.width() / 2.0,
                Color32::BLACK,
                ui.visuals().noninteractive().bg_stroke,
            );
            ui.painter()
                .line_segment([rect.left_center(), rect.right_center()], ui.visuals().noninteractive().bg_stroke);
            ui.painter()
                .line_segment([rect.center_top(), rect.center_bottom()], ui.visuals().noninteractive().bg_stroke);
            ui.painter().circle(
                rect.center(),
                (rect.width() / 2.0) * self.stick_bounds.deadzone,
                Color32::TRANSPARENT,
                Stroke::new(1.0, Color32::from_rgb(0, 128, 200)),
            );
            ui.painter().circle(
                rect.center(),
                (rect.width() / 2.0) * self.stick_bounds.stick_max,
                Color32::TRANSPARENT,
                Stroke::new(1.0, Color32::GOLD),
            );
            let plt_x = buffer[99][0] * rect.width() / 2.0;
            let plt_y = 0.0 - buffer[99][1] * rect.width() / 2.0;

            ui.painter().circle_filled(rect.center() + vec2(plt_x, plt_y), 4.0, Color32::WHITE);

            graph::graph(&buffer, self.id_mod, ui, |a, _| {
                let length = f32::sqrt(a[0] * a[0] + a[1] * a[1]);

                let x = if length < self.stick_bounds.deadzone { 0.0 } else { a[0] / self.stick_bounds.stick_max };
                let y = if length < self.stick_bounds.deadzone { 0.0 } else { a[1] / self.stick_bounds.stick_max };

                [x, y, a[2], a[3]]
            });
        });
    }

    fn render_failure(&self, ctx: &eframe::egui::Context, failure: &VRSystemFailure) {
        let rect = ctx.screen_rect().shrink2(vec2(0.0, (ctx.screen_rect().height() / 2.0) - 100.0));

        let painter = ctx.layer_painter(LayerId::new(Order::Foreground, Id::new("FailureMessage")));

        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(128, 0, 0, 16));

        painter.line_segment([rect.min, rect.right_top()], Stroke::new(2.0, Color32::RED));
        painter.line_segment([rect.max, rect.left_bottom()], Stroke::new(2.0, Color32::RED));

        let err =
            painter.layout(format!("{}", failure), FontId::proportional(24.0), Color32::WHITE, rect.width() * 0.75);

        painter.galley(rect.center() - err.rect.size() / 2.0, err, Color32::WHITE);
    }
}

#[profiling::all_functions]
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
            if self.startup_failure.is_some() {
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

            ui.scope(|ui| {
                if self.runtime_failure.is_some() {
                    ui.disable();
                }
                self.main_content(ui);
            });

            profiling::finish_frame!();
        });

        if let Some(failure) = &self.startup_failure {
            self.render_failure(ctx, failure);
        } else if let Some(failure) = &self.runtime_failure {
            self.render_failure(ctx, failure);
        }
    }

    fn on_exit(&mut self, _ctx: Option<&eframe::glow::Context>) {
        let _ = self.tx.send(UI2VR::Shutdown);
        println!("Frontend shut down");
    }
}
