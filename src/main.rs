use cpal::traits::{DeviceTrait, HostTrait};
use std::thread;

mod busses;
mod router;
mod tracks;

use crate::router::Router;

fn main() {
    println!("Hello, world!");
    let host = cpal::default_host();

    let in_device = host.default_input_device().unwrap();
    let in_device_name = in_device.name().unwrap();
    println!("in device: {}", in_device_name);

    let in_config = in_device.default_input_config().unwrap().config();

    let out_device = host.default_output_device().unwrap();
    let out_device_name = out_device.name().unwrap();
    println!("out device: {}", out_device_name);
    let out_config = out_device.default_output_config().unwrap().config();

    println!("Channels: {}", in_config.channels);
    println!("Channels: {}", out_config.channels);

    let sample_format = in_device.default_input_config().unwrap().sample_format();
    match sample_format {
        cpal::SampleFormat::F32 => println!("f32"),
        cpal::SampleFormat::I16 => println!("i16"),
        cpal::SampleFormat::U16 => println!("i16"),
    }

    let mut router = Router::<f32>::new(
        host,
        in_config,
        out_config,
        in_device_name,
        out_device_name,
        // "Analogue 1 + 2 (Focusrite Usb Audio)".to_string(),
        sample_format,
    );
    router.new_input_bus(vec![1]);
    router.new_input_bus(vec![1, 2]);

    router.new_output_bus(vec![1, 2]);

    router.new_track("Tractor".to_string(), 0, 0);
    router.new_track("Tractor1".to_string(), 1, 0);
    // router.new_track("Tractor2".to_string(), 0, 0);

    router.set_recording(0, true);
    router.set_recording(1, true);

    router.set_monitor(0, true);
    router.set_monitor(1, true);
    // thread::sleep(std::time::Duration::from_secs(10));

    router.monitor();
    router.record();
    thread::sleep(std::time::Duration::from_secs(10));
    router.stop_recording();
    router.stop_monitor();
    println!("Done!");

    router.monitor();
    router.record();
    thread::sleep(std::time::Duration::from_secs(10));
    router.stop_recording();
    router.stop_monitor();
    println!("Done!");
}
