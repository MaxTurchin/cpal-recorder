// struct TrackConfig
use crate::{recorder, utils};

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Host;

pub struct Track {
    name: String,
    files: Vec<String>,
    input_channels: Vec<u8>,
    output_channels: Vec<u8>,
    mono_stereo: utils::MonoStereo,
    //TODO: output_channels: Vec<i8>,
    monitor_config: recorder::MonitorConfig,
    recorder_config: recorder::RecorderConfig,
    monitor: recorder::Monitor,
    recorder: recorder::Recorder,
}

impl Track {
    pub fn new(
        name: String,
        host: &Host,
        input_device_name: String,
        output_device_name: String,
        input_channels: Vec<u8>,
        output_channels: Vec<u8>,
        mono_stereo: utils::MonoStereo,
    ) -> Track {
        let file = format!("{}_1.wav", name);
        let files = vec![file.clone()];

        let input_device = host
            .input_devices()
            .unwrap()
            .find(|x| x.name().map(|y| y == input_device_name).unwrap_or(false))
            .unwrap();

        let output_device = host
            .output_devices()
            .unwrap()
            .find(|x| x.name().map(|y| y == output_device_name).unwrap_or(false))
            .unwrap();

        let monitor_conf = recorder::MonitorConfig::new(
            input_device,
            output_device,
            mono_stereo.clone(),
            input_channels.clone(),
            output_channels.clone(),
        );

        let input_device = host
            .input_devices()
            .unwrap()
            .find(|x| x.name().map(|y| y == input_device_name).unwrap_or(false))
            .unwrap();

        let rec_conf = recorder::RecorderConfig::new(
            file,
            input_device,
            mono_stereo.clone(),
            input_channels.clone(),
        );
        let rec = recorder::Recorder::new(&rec_conf);
        let mon = recorder::Monitor::new(&monitor_conf);

        Track {
            name: name,
            files: files,
            input_channels: input_channels,
            output_channels: output_channels,
            mono_stereo: mono_stereo,

            monitor_config: monitor_conf,
            recorder_config: rec_conf,
            monitor: mon,
            recorder: rec,
        }
    }

    pub fn start_monitor(&self) {
        self.monitor.start_monitor();
    }

    pub fn stop_monitor(&mut self) {
        self.monitor.stop_monitor();
        self.monitor = recorder::Monitor::new(&self.monitor_config);
    }

    pub fn start_recording(&self) {
        self.recorder.start_recording();
    }

    pub fn stop_recording(&mut self) {
        self.recorder.stop_recording();
        self.add_new_wav();
        self.recorder = recorder::Recorder::new(&self.recorder_config);
    }

    fn add_new_wav(&mut self) {
        let len = self.files.len();
        let new_wav = format!("{}_{}.wav", self.name, len + 1);
        self.recorder_config.wav_path = new_wav.clone();
        self.files.push(new_wav);
    }
}
