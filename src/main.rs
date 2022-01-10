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
    let output_config = output.default_output_config().unwrap();

    let sample_format = input_config.sample_format();

    let input_config = input_config.config();
    let output_config = output_config.config();


    let monitor_conf = recorder::MonitorConfig {
        input, output, input_config, output_config, sample_format
    };

    let input = host.default_input_device().unwrap();
    let input_config = input.default_input_config().unwrap();
    let sample_format = input_config.sample_format();
    let input_config = input_config.config();


    let recorder_conf = recorder::RecorderConfig {
        wav_path, input, input_config, sample_format
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
