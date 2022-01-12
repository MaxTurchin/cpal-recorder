use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, Device, SampleFormat, Stream, StreamConfig};
use hound;
use std::sync::{Arc, Mutex};

use crate::utils;

pub struct RecorderConfig {
    pub wav_path: String,
    input: Device,
    input_config: StreamConfig,
    sample_format: SampleFormat,
    mono_stereo: utils::MonoStereo,
    input_channels: Vec<u8>,
}

//TODO: validation of buffer_size
impl RecorderConfig {
    pub fn new(
        fpath: String,
        input_device: Device,
        mono_stereo: utils::MonoStereo,
        input_channels: Vec<u8>,
    ) -> RecorderConfig {
        let default_input_conf = input_device.default_input_config().unwrap();
        let sample_f = default_input_conf.sample_format();

        let input_conf = default_input_conf.config();

        return RecorderConfig {
            wav_path: fpath,
            input: input_device,
            input_config: input_conf,
            sample_format: sample_f,
            mono_stereo: mono_stereo,
            input_channels: input_channels,
        };
    }

    // pub fn set_buffer_size(&mut self, buffer_s: &BufferSize) {
    //     self.input_config.buffer_size = buffer_s.clone();
    // }
}

//TODO: validation of buffer_size
pub struct MonitorConfig {
    input: Device,
    output: Device,
    input_config: cpal::StreamConfig,
    output_config: cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    mono_stereo: utils::MonoStereo,
    input_channels: Vec<u8>,
    output_channels: Vec<u8>,
}

impl MonitorConfig {
    pub fn new(
        input_device: Device,
        output_device: Device,
        mono_stereo: utils::MonoStereo,
        input_channels: Vec<u8>,
        output_channels: Vec<u8>,
    ) -> MonitorConfig {
        let default_input_conf = input_device.default_input_config().unwrap();
        let default_output_conf = output_device.default_output_config().unwrap();

        let sample_f = default_input_conf.sample_format();

        let input_conf = default_input_conf.config();
        let output_conf = default_output_conf.config();

        return MonitorConfig {
            input: input_device,
            output: output_device,
            input_config: input_conf,
            output_config: output_conf,
            sample_format: sample_f,
            mono_stereo: mono_stereo,
            input_channels: input_channels,
            output_channels: output_channels,
        };
    }

    pub fn set_buffer_size(&mut self, buffer_s: &BufferSize) {
        self.input_config.buffer_size = buffer_s.clone();
        self.output_config.buffer_size = buffer_s.clone();
    }
}

pub struct Recorder {
    pub input_stream: Box<Stream>,
}

impl Recorder {
    pub fn new(conf: &RecorderConfig) -> Recorder {
        let path = conf.wav_path.clone();
        let wav_spec = utils::wav_spec_from_config(&conf.input_config, &conf.sample_format);

        let writer = hound::WavWriter::create(path, wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        return Recorder {
            input_stream: Box::new(utils::make_write_stream::<f32, f32>(
                &conf.input_config,
                &conf.input,
                &conf.mono_stereo,
                &conf.input_channels,
                &conf.sample_format,
                &writer,
            )),
        };
    }

    pub fn start_recording(&self) {
        self.input_stream.play().unwrap();
    }

    pub fn stop_recording(&mut self) {
        drop(&*self.input_stream);
    }
}

pub struct Monitor {
    input_stream: Box<Stream>,
    output_stream: Box<Stream>,
}

impl Monitor {
    pub fn new(conf: &MonitorConfig) -> Monitor {
        let (input_stream, output_stream) = utils::make_monitor_streams(
            &conf.input_config,
            &conf.output_config,
            &conf.sample_format,
            &conf.input,
            &conf.output,
            &conf.mono_stereo,
            &conf.input_channels,
            &conf.output_channels,
        );
        return Monitor {
            input_stream: Box::new(input_stream),
            output_stream: Box::new(output_stream),
        };
    }

    pub fn start_monitor(&self) {
        self.input_stream.play().unwrap();
        self.output_stream.play().unwrap();
    }

    pub fn stop_monitor(&mut self) {
        drop(&*self.input_stream);
        drop(&*self.output_stream);
    }
}
