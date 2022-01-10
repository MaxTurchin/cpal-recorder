
use std::sync::{
    Arc,
    Mutex
};

use cpal::traits::{
    DeviceTrait,
    HostTrait
};

use cpal::{
    StreamConfig,
    SampleFormat,
    Host,
    Device
};
use cpal::Stream;

use hound;

use crate::utils;


pub struct Recorder {
    wav_path: String,
    host:     Host,
    input:    Device,
    output:   Device,

    input_config:  StreamConfig,
    output_config: StreamConfig,
    sample_format: SampleFormat
}


impl Recorder {
    pub fn new_default() -> Recorder {
        let wav_path = "./wav.wav".to_string();

        let host = cpal::default_host();
        
        let input = host.default_input_device().unwrap();
        let output = host.default_output_device().unwrap();

        let input_config = input.default_input_config().unwrap();
        let output_config = output.default_output_config().unwrap();

        let sample_format = input_config.sample_format();

        let input_config = input_config.config();
        let output_config = output_config.config();

        // let input_config = cpal::StreamConfig {
        //     channels: 1 as u16,
        //     sample_rate: cpal::SampleRate(44100 as u32),
        //     buffer_size: cpal::BufferSize::Default
        // };

        // let output_config = cpal::StreamConfig {
        //     channels: 2 as u16,
        //     sample_rate: cpal::SampleRate(44100 as u32),
        //     buffer_size: cpal::BufferSize::Default
        // };

        //temporary for ASIO testing:
        // let host = cpal::host_from_id(cpal::HostId::Asio).unwrap();
        // let input = host.input_devices().unwrap()
        //             .find(|x| x.name().map(|y| y == "Focusrite USB ASIO").unwrap_or(false))
        //             .unwrap();
        // let output = host.output_devices().unwrap()
        //             .find(|x| x.name().map(|y| y == "Focusrite USB ASIO").unwrap_or(false))
        //             .unwrap();

        // let input_config = input.default_input_config().unwrap();
        // let output_config = output.default_output_config().unwrap();
       
        return Recorder {
            wav_path: wav_path,

            host:   host,
            input:  input,
            output: output,

            input_config:  input_config,
            output_config: output_config,
            sample_format: sample_format
        }
    }


    pub fn record(&self) -> Stream {
        let path = self.wav_path.clone();
        let wav_spec = utils::wav_spec_from_config(&self.input_config,
                                                   &self.sample_format);

        let writer = hound::WavWriter::create(path, wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        return utils::make_write_stream(&self.input_config,
                                        &self.input, 
                                        &self.sample_format,
                                        &writer);
    }


    pub fn monitor(&self) -> (Stream, Stream){
        return utils:: make_monitor_streams(&self.input_config,
                                            &self.output_config,
                                            &self.sample_format,
                                            &self.input,
                                            &self.output);
    }


    pub fn show(self) {
        println!("Recorder:");
        let host_name = self.host.id().name();
        let input_name = self.input.name().unwrap();
        let output_name = self.output.name().unwrap();

        println!("\thost: {}\n\tinput device: {}\n\toutput device: {}\n",
                 host_name, input_name, output_name);

        println!("Input Config:");
        utils::show_config(&self.input_config);
        println!("Output Config:");
        utils::show_config(&self.output_config);
    }
}
