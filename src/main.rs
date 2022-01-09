mod recorder;
mod utils;

use std::thread;
use std::time;

use cpal::traits::StreamTrait;
use cpal::Stream;

use crossterm::event::{read, Event, KeyCode, KeyEvent};


fn main() {
    utils::show_hosts();
    utils::show_devices();

    let rec = recorder::Recorder::new_default();
    rec.show();

    println!("...");
    println!("...");
    println!("...");

    let rec = recorder::Recorder::new_default();

    let (monitor_input, monitor_output): (Stream, Stream) = rec.monitor();
    let record = rec.record();

    println!("Rec...");

    monitor_input.play().unwrap();
    monitor_output.play().unwrap();
    record.play().unwrap();

    loop {
        thread::sleep(time::Duration::from_millis(10));
        match read().unwrap() {
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: _no_modifiers
            }) => {
                    drop(record);
                    drop(monitor_input);
                    drop(monitor_output);
                    break;
                },

            _ => ()
        }
    }
    println!("Done...");
}
