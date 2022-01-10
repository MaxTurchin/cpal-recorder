mod recorder;
mod utils;

use std::{
    thread,
    time
};
use cpal::traits::{
    HostTrait,
    DeviceTrait
};
use crossterm::event::{
    read,
    Event,
    KeyCode,
    KeyEvent
};


fn main() {
    utils::show_hosts();
    utils::show_devices();

    let wav_path = "./wav.wav".to_string();
    let host = cpal::default_host();
    let input = host.default_input_device().unwrap();
    let output = host.default_output_device().unwrap();
    let input_config = input.default_input_config().unwrap();
    let sample_format = input_config.sample_format();

    let mono_stereo = utils::MonoStereo::MONO;
    let input_channels = vec![1];


    let monitor_conf = recorder::MonitorConfig {
        input:          host.default_input_device().unwrap(),
        output:         host.default_output_device().unwrap(),
        input_config:   input.default_input_config().unwrap().config(),
        output_config:  output.default_output_config().unwrap().config(),
        sample_format:  sample_format,
        mono_stereo:    mono_stereo.clone(),
        input_channels: input_channels.clone()
    };


    let recorder_conf = recorder::RecorderConfig {
        wav_path:       wav_path,
        input:          host.default_input_device().unwrap(),
        input_config:   input.default_input_config().unwrap().config(),
        sample_format:  sample_format,
        mono_stereo:    mono_stereo.clone(),
        input_channels: input_channels
    };



    println!("...");
    println!("...");
    println!("...");

    loop {
        match read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: _no_modifiers
            }) => {
                println!("Rec...");
                // rec.start_recording();
                // monitor.start_monitor();
                let mon = recorder::Monitor::start_monitor(&monitor_conf);
                let rec = recorder::Recorder::start_recording(&recorder_conf);

                loop {
                    thread::sleep(time::Duration::from_millis(10));
                    match read().unwrap() {
                        Event::Key(KeyEvent {
                            code: KeyCode::Enter,
                            modifiers: _no_modifiers
                        }) => {
                                rec.stop_recording();
                                mon.stop_monitor();
                                break;
                            }
                        _ => ()
                    }
                }
                println!("Done...");
            },
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: _no_modifiers
            }) => {
                break;
            }
            _ => ()
        }
    }
}
