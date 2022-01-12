mod recorder;
mod track;
mod utils;

use cpal::traits::{DeviceTrait, HostTrait};
use crossterm::event::{read, Event, KeyCode, KeyEvent};
use std::{thread, time};

fn main() {
    utils::show_hosts();
    utils::show_devices();

    let host = cpal::default_host();
    let mono_stereo = utils::MonoStereo::STEREO;

    println!(
        "{:?}",
        host.default_input_device()
            .unwrap()
            .default_input_config()
            .unwrap()
    );
    println!(
        "{:?}",
        host.default_output_device()
            .unwrap()
            .default_output_config()
            .unwrap()
    );


    let mut track = track::Track::new(
        "track".to_string(),
        &host,
        "Analogue 1 + 2 (Focusrite Usb Audio)".to_string(),
        "Speakers (Focusrite Usb Audio)".to_string(),
        vec![1, 2],
        mono_stereo.clone(),
    );

    println!("...");
    println!("...");
    println!("...");

    loop {
        match read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: _no_modifiers,
            }) => {
                println!("Rec...");
                track.start_monitor();
                track.start_recording();

                loop {
                    thread::sleep(time::Duration::from_millis(10));
                    match read().unwrap() {
                        Event::Key(KeyEvent {
                            code: KeyCode::Enter,
                            modifiers: _no_modifiers,
                        }) => {
                            track.stop_recording();
                            track.stop_monitor();
                            break;
                        }
                        _ => (),
                    }
                }
                println!("Done...");
            }
            Event::Key(KeyEvent {
                code: KeyCode::Esc,
                modifiers: _no_modifiers,
            }) => {
                break;
            }
            _ => (),
        }
    }
}
