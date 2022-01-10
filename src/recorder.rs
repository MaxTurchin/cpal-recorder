
use std::sync::{
    Arc,
    Mutex
};
use cpal::{
    StreamConfig,
    SampleFormat,
    Stream,
    Device
};
use cpal::traits::StreamTrait;
use hound;

use crate::utils;


pub struct RecorderConfig {
    pub wav_path: String,
    pub input:    Device,
    pub input_config:  StreamConfig,
    pub sample_format: SampleFormat
}


pub struct MonitorConfig {
    pub input:  Device,
    pub output: Device,
    pub input_config:  cpal::StreamConfig,
    pub output_config: cpal::StreamConfig,
    pub sample_format: cpal::SampleFormat
}


pub struct Recorder {
    input_stream: Stream
}

impl Recorder {
    fn _new(conf: &RecorderConfig) -> Recorder {
        let path = conf.wav_path.clone();
        let wav_spec = utils::wav_spec_from_config(&conf.input_config,
                                                   &conf.sample_format);

        let writer = hound::WavWriter::create(path, wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        return Recorder {
            input_stream: utils::make_write_stream(&conf.input_config,
                                                   &conf.input,
                                                   &conf.sample_format,
                                                   &writer)
        };
    }

    pub fn start_recording(conf: &RecorderConfig) -> Recorder {
        let rec = Recorder::_new(conf);
        rec.input_stream.play().unwrap();

        return rec;
    }

    pub fn stop_recording(self) {
        drop(self.input_stream);
    }
}


pub struct Monitor {
    input_stream: Stream,
    output_stream: Stream
}

impl Monitor {
    fn _new(conf: &MonitorConfig) -> Monitor {

        let (input_stream, output_stream) = utils::make_monitor_streams(&conf.input_config,
                                                                        &conf.output_config,
                                                                        &conf.sample_format,
                                                                        &conf.input,
                                                                        &conf.output);
        return Monitor {
            input_stream: input_stream,
            output_stream: output_stream
        };
    }

    pub fn start_monitor(conf: &MonitorConfig) -> Monitor{
        let monitor = Monitor::_new(conf);
        monitor.input_stream.play().unwrap();
        monitor.output_stream.play().unwrap();

        return monitor;
    }

    pub fn stop_monitor(self) {
        drop(self.input_stream);
        drop(self.output_stream);
    }
}
