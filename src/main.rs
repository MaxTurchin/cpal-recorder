use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host, HostId, StreamConfig, SupportedInputConfigs, SupportedOutputConfigs};
use std::borrow::Cow;
use std::thread;

mod busses;
mod router;
mod tracks;
mod utils;

use crate::router::Router;

use eframe::egui::containers::ScrollArea;
use eframe::egui::containers::Window;
use eframe::egui::{
    Align, ComboBox, FontData, FontDefinitions, InnerResponse, Response, TextEdit, Vec2,
};
use eframe::run_native;
use eframe::NativeOptions;
use eframe::{egui, epi};

pub struct TrackUi {
    id: u8,
    name: String,
    is_monitored: bool,
    is_recorded: bool,
    state: (bool, bool), //(is_rec, is_monitored)
}

impl Default for TrackUi {
    fn default() -> Self {
        Self {
            id: 0,
            name: "TrackUi".to_string(),
            is_monitored: false,
            is_recorded: false,
            state: (false, false), //(is_rec, is_monitored)
        }
    }
}

impl TrackUi {
    fn new(id: u8, name: String, is_monitored: bool, is_recorded: bool) -> Self {
        let state = (is_recorded, is_monitored);
        Self {
            id,
            name,
            is_recorded,
            is_monitored,
            state,
        }
    }

    fn show(&mut self, ui: &mut eframe::egui::Ui, app_router: &mut router::Router<f32>) {
        ui.horizontal(|ui| {
            ui.label(format!("{}.", self.id.to_string()));
            ui.vertical(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.name).frame(false));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.is_monitored, "Monitored");
                    ui.checkbox(&mut self.is_recorded, "Rec.");
                });
            });
        });
        self.apply_changes(app_router);
    }

    fn apply_changes(&mut self, app_router: &mut router::Router<f32>) {
        let (rec_changed, monitor_changed) = self.get_changed();
        if rec_changed {
            app_router.set_recording(self.id, self.is_recorded);
        }
        if monitor_changed {
            app_router.set_monitor(self.id, self.is_monitored);
            app_router.stop_monitor();
            app_router.monitor();

            // if self.is_monitored {
            //     app_router.stop_monitor();
            //     app_router.monitor();
            // } else {
            //     app_router.stop_monitor();
            //     app_router.monitor();
            // }
        }
    }

    fn get_changed(&mut self) -> (bool, bool) {
        //(is_rec_changed, is_monitored_changed)
        let mut res = (false, false);
        if self.state.0 != self.is_recorded {
            self.state.0 = self.is_recorded;
            res.0 = true;
        }
        if self.state.1 != self.is_monitored {
            self.state.1 = self.is_monitored;
            res.1 = true;
        }
        return res;
    }
}

pub struct TrackListUi {
    add_track_window: AddTrack,
    track_list: Vec<TrackUi>,
}

impl TrackListUi {
    pub fn new() -> Self {
        Self {
            add_track_window: AddTrack::default(),
            track_list: Vec::<TrackUi>::new(),
        }
    }

    fn update_track_lst(&mut self, app_router: &Router<f32>) {
        let current_lst = app_router.get_tracks();
        for item in current_lst {
            let t_as_tup = item.as_tup(); //(id, name, is_rec, is_monitored)
            match self.track_list.iter().find(|x| x.id == t_as_tup.0) {
                Some(_) => continue,
                None => self
                    .track_list
                    .push(TrackUi::new(t_as_tup.0, t_as_tup.1, t_as_tup.2, t_as_tup.3)),
            }
        }
    }

    fn get_track_list(
        &mut self,
        ctx: &egui::CtxRef,
        ui: &mut eframe::egui::Ui,
        app_router: &mut Option<Router<f32>>,
    ) {
        let rout = match app_router {
            Some(r) => r,
            None => return (),
        };
        self.update_track_lst(rout);

        ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for item in self.track_list.iter_mut() {
                    item.show(ui, rout);
                }
                ui.separator();
                self.add_track_window.get_window(ctx, rout);
                if ui.button("Add Track +").clicked() {
                    self.add_track_window.open = true;
                }
            });
    }
}

impl Default for TrackListUi {
    fn default() -> Self {
        let mut t_list = Vec::<TrackUi>::new();
        for i in 0..20 {
            t_list.push(TrackUi::default());
        }

        let track_window = AddTrack::default();
        Self {
            add_track_window: track_window,
            track_list: t_list,
        }
    }
}

pub struct AddTrack {
    track_name: String,
    selected_in_channels: Vec<(bool, u8)>, //(boolean representing checkbox, u8 in channel id)
    selected_out_channels: Vec<(bool, u8)>, //(boolean representing checkbox, u8 out channel id)
    open: bool,
}

impl AddTrack {
    fn reset_window(&mut self) {
        self.track_name = String::new();
        self.selected_in_channels = Vec::<(bool, u8)>::new();
        self.selected_out_channels = Vec::<(bool, u8)>::new();
    }

    fn update_selection_lst(&mut self, app_router: &mut Router<f32>) {
        let (input_chs, output_chs) = app_router.get_io_channels();

        let mut select_in_chs = Vec::<(bool, u8)>::new();
        let mut select_out_chs = Vec::<(bool, u8)>::new();
        input_chs
            .iter()
            .for_each(|x| select_in_chs.push((false, *x)));
        output_chs
            .iter()
            .for_each(|x| select_out_chs.push((false, *x)));

        self.selected_in_channels = select_in_chs;
        self.selected_out_channels = select_out_chs;
    }

    fn get_window(
        &mut self,
        ctx: &egui::CtxRef,
        app_router: &mut Router<f32>,
    ) -> Option<InnerResponse<Option<()>>> {
        if self.selected_in_channels.is_empty() || self.selected_out_channels.is_empty() {
            self.update_selection_lst(app_router);
        }

        let mut close = false;

        let window = Window::new("Add Track")
            .open(&mut self.open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Track Name:");
                        ui.label("Bus Config:");
                    });
                    ui.vertical(|ui| {
                        ui.text_edit_singleline(&mut self.track_name);
                        egui::CollapsingHeader::new("Input Channels").show(ui, |ui| {
                            for in_ch in self.selected_in_channels.iter_mut() {
                                let label_txt = format!("Input {}", in_ch.1);
                                ui.checkbox(&mut in_ch.0, label_txt);
                                // ui.selectable_value(&mut self.selected_in_channel, in_ch, label_txt);
                            }
                        });
                        egui::CollapsingHeader::new("Output Channels").show(ui, |ui| {
                            for out_ch in self.selected_out_channels.iter_mut() {
                                let label_txt = format!("Output {}", out_ch.1);
                                ui.checkbox(&mut out_ch.0, label_txt);
                                // ui.selectable_value(&mut self.selected_out_channel, out_ch, label_txt);
                            }
                        });
                    });
                });

                let (mut in_chs, mut out_chs) = (Vec::<u8>::new(), Vec::<u8>::new());
                self.selected_in_channels.iter().for_each(|x| {
                    if x.0 {
                        in_chs.push(x.1);
                    }
                });
                self.selected_out_channels.iter().for_each(|x| {
                    if x.0 {
                        out_chs.push(x.1);
                    }
                });
                let mut enabled = true;
                if in_chs.is_empty() || out_chs.is_empty() || self.track_name.is_empty() {
                    enabled = false;
                }

                if ui
                    .add_enabled(enabled, egui::Button::new("Add +"))
                    .on_disabled_hover_text(
                        "Input/Output channels must be selected and Track Name cannot be empty",
                    )
                    .clicked()
                {
                    let (in_bus, out_bus) = (
                        app_router.new_input_bus(in_chs),
                        app_router.new_output_bus(out_chs),
                    );
                    app_router.new_track(self.track_name.clone(), in_bus, out_bus);
                    close = true;
                }
            });

        if close {
            self.open = false;
            self.reset_window();
        }
        return window;
    }
}

impl Default for AddTrack {
    fn default() -> Self {
        Self {
            track_name: "".to_string(),
            selected_in_channels: Vec::<(bool, u8)>::new(),
            selected_out_channels: Vec::<(bool, u8)>::new(),
            open: false,
        }
    }
}

pub struct StudioSetup {
    host_ids: Vec<HostId>,
    selected_host_id: HostId,

    in_devices: Vec<Device>,
    selected_in_device: String,

    out_devices: Vec<Device>,
    selected_out_device: String,

    selected_sample_format: cpal::SampleFormat,

    open: bool,
}

impl Default for StudioSetup {
    fn default() -> Self {
        let default_host_id = cpal::default_host().id();
        let default_host = cpal::host_from_id(default_host_id);
        let default_sample_format = default_host
            .unwrap()
            .default_input_device()
            .unwrap()
            .default_input_config()
            .unwrap()
            .sample_format();
        let (in_devices, out_devices) = utils::get_host_devices(default_host_id);

        Self {
            host_ids: utils::get_host_ids(),
            selected_host_id: default_host_id,
            in_devices: in_devices,
            selected_in_device: String::new(),
            out_devices: out_devices,
            selected_out_device: String::new(),
            selected_sample_format: default_sample_format,
            open: true,
        }
    }
}

impl StudioSetup {
    fn get_window(
        &mut self,
        ctx: &egui::CtxRef,
        app_router: &mut Option<Router<f32>>,
    ) -> Option<InnerResponse<Option<()>>> {
        let mut close_window = false;
        let window = Window::new("Studio Setup")
            .open(&mut self.open)
            .show(ctx, |ui| {
                ComboBox::from_label("Host")
                    .width(300.)
                    .selected_text(format!("{:?}", self.selected_host_id))
                    .show_ui(ui, |ui| {
                        for host_id in self.host_ids.iter() {
                            ui.selectable_value(
                                &mut self.selected_host_id,
                                *host_id,
                                host_id.name(),
                            );
                        }
                    });
                ComboBox::from_label("Input Device")
                    .width(300.)
                    .selected_text(format!("{:?}", self.selected_in_device))
                    .show_ui(ui, |ui| {
                        for input in self.in_devices.iter() {
                            let d_name = input.name().unwrap();
                            ui.selectable_value(
                                &mut self.selected_in_device,
                                d_name.clone(),
                                d_name,
                            );
                        }
                    });
                ComboBox::from_label("Output Device")
                    .width(300.)
                    .selected_text(format!("{:?}", self.selected_out_device))
                    .show_ui(ui, |ui| {
                        for output in self.out_devices.iter() {
                            let d_name = output.name().unwrap();
                            ui.selectable_value(
                                &mut self.selected_out_device,
                                d_name.clone(),
                                d_name,
                            );
                        }
                    });
                ComboBox::from_label("Sample Format")
                    .width(300.)
                    .selected_text(format!("{:?}", self.selected_sample_format))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.selected_sample_format,
                            cpal::SampleFormat::F32,
                            "f32",
                        );
                        ui.selectable_value(
                            &mut self.selected_sample_format,
                            cpal::SampleFormat::I16,
                            "i16",
                        );
                        ui.selectable_value(
                            &mut self.selected_sample_format,
                            cpal::SampleFormat::U16,
                            "u16",
                        );
                    });

                let mut enabled = true;
                if self.selected_out_device.is_empty() || self.selected_in_device.is_empty() {
                    enabled = false;
                }

                if ui
                    .add_enabled(enabled, egui::Button::new("Apply"))
                    .on_disabled_hover_text("Input/Output devices must be selected")
                    .clicked()
                {
                    let host = cpal::host_from_id(self.selected_host_id).unwrap();
                    let in_device =
                        utils::get_input_device_by_name(&host, &self.selected_in_device);
                    let out_device =
                        utils::get_output_device_by_name(&host, &self.selected_out_device);
                    let in_conf = in_device.default_input_config().unwrap().config();
                    let out_conf = out_device.default_output_config().unwrap().config();

                    *app_router = Option::Some(Router::new(
                        host,
                        in_conf,
                        out_conf,
                        self.selected_in_device.clone(),
                        self.selected_out_device.clone(),
                        self.selected_sample_format,
                    ));

                    close_window = true;
                }
            });

        if close_window {
            self.open = false;
        }
        return window;
    }
}

pub struct TransportUi;

impl TransportUi {
    pub fn get_transport(
        &mut self,
        ui: &mut egui::Ui,
        app_router: &mut Option<Router<f32>>,
    ) -> InnerResponse<()> {
        ui.with_layout(egui::Layout::right_to_left(), |ui| {
            let rout = match app_router {
                Some(r) => r,
                None => return,
            };
            if ui.button("Stop").clicked() {
                rout.stop_recording();
            }
            if ui.button("Play").clicked() {}
            if ui.button("Rec.").clicked() {
                rout.record();
            }
        })
    }
}

pub struct ToolbarUi;

impl ToolbarUi {
    pub fn get_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        setup: &mut StudioSetup,
    ) -> InnerResponse<Option<()>> {
        ui.menu_button("Studio", |ui| {
            self.get_nested_menus(ui, setup);
        })
    }

    fn get_nested_menus(&mut self, ui: &mut egui::Ui, setup: &mut StudioSetup) -> () {
        if ui.button("Setup").clicked() {
            setup.open = true;
        }
    }
}

pub struct CpalRecorder {
    setup: StudioSetup,
    track_list: TrackListUi,
    transport: TransportUi,
    toolbar: ToolbarUi,
    router: Option<Router<f32>>,
}

impl CpalRecorder {
    fn conf_fonts(&self, ctx: &egui::CtxRef) {
        let mut font_def = FontDefinitions::default();
        font_def.font_data.insert(
            "Mukta".to_string(),
            FontData {
                font: Cow::Borrowed(include_bytes!("../Mukta-Medium.ttf")),
                index: 0 as u32,
            },
        );

        font_def.family_and_size.insert(
            eframe::egui::TextStyle::Heading,
            (egui::FontFamily::Proportional, 30.),
        );
        font_def.family_and_size.insert(
            eframe::egui::TextStyle::Body,
            (egui::FontFamily::Proportional, 25.),
        );
        font_def.family_and_size.insert(
            eframe::egui::TextStyle::Button,
            (egui::FontFamily::Proportional, 20.),
        );

        font_def
            .fonts_for_family
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "Mukta".to_string());
        ctx.set_fonts(font_def);
    }
}

impl Default for CpalRecorder {
    fn default() -> Self {
        Self {
            setup: StudioSetup::default(),
            track_list: TrackListUi::new(),
            transport: TransportUi {},
            toolbar: ToolbarUi {},
            router: None,
        }
    }
}

impl epi::App for CpalRecorder {
    fn name(&self) -> &str {
        "cpal-Recorder"
    }

    fn setup(
        &mut self,
        ctx: &egui::CtxRef,
        frame: &epi::Frame,
        _storage: Option<&dyn eframe::epi::Storage>,
    ) {
        self.conf_fonts(ctx);
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &epi::Frame) {
        self.setup.get_window(ctx, &mut self.router);
        egui::TopBottomPanel::top("Toolbar").show(ctx, |ui| {
            self.toolbar.get_toolbar(ui, &mut self.setup);
        });
        egui::TopBottomPanel::bottom("TransportUi").show(ctx, |ui| {
            self.transport.get_transport(ui, &mut self.router);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.track_list.get_track_list(ctx, ui, &mut self.router);
        });
    }
}

fn main() {
    let app: CpalRecorder = CpalRecorder::default();
    let mut win_opts = NativeOptions::default();
    win_opts.initial_window_size = Some(Vec2::new(1280., 720.));
    run_native(Box::new(app), win_opts);
}
