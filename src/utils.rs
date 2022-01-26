use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{
    available_hosts, host_from_id, Device, Host, HostId, SampleFormat, StreamConfig,
    SupportedInputConfigs, SupportedOutputConfigs,
};
use multiqueue;

pub fn get_host_ids() -> Vec<HostId> {
    let mut host_ids = Vec::<HostId>::new();
    for host_id in available_hosts().iter() {
        host_ids.push(*host_id);
    }
    host_ids
}

pub fn get_host_devices(host_id: HostId) -> (Vec<Device>, Vec<Device>) {
    let host = match host_from_id(host_id) {
        Ok(h) => h,
        Err(e) => panic!("utils::get_host_devices(): Oh no! {}", e),
    };
    let mut in_devices: Vec<Device> = Vec::<Device>::new();
    let mut out_devices: Vec<Device> = Vec::<Device>::new();

    for input in host.input_devices().unwrap() {
        in_devices.push(input);
    }
    for out in host.output_devices().unwrap() {
        out_devices.push(out);
    }
    (in_devices, out_devices)
}

pub fn get_supported_configs(
    host: &Host,
    input: &String,
    output: &String,
) -> (SupportedInputConfigs, SupportedOutputConfigs) {
    let (input, output) = (
        get_input_device_by_name(host, input),
        get_output_device_by_name(host, output),
    );
    let in_configs = match input.supported_input_configs() {
        Ok(iter) => iter,
        Err(e) => panic!("utils::get_supported_configs().in_configs: Oh no! {}", e),
    };
    let out_configs = match output.supported_output_configs() {
        Ok(iter) => iter,
        Err(e) => panic!("utils::get_supported_configs().out_configs: Oh no! {}", e),
    };
    (in_configs, out_configs)
}

pub fn get_input_device_by_name(host: &Host, device_name: &String) -> Device {
    let mut devices = match host.input_devices() {
        Ok(d) => d,
        Err(e) => panic!("Input devices Not Found: {}", e),
    };
    devices
        .find(|x| x.name().map(|y| y == *device_name).unwrap_or(false))
        .unwrap()
}

pub fn get_output_device_by_name(host: &Host, device_name: &String) -> Device {
    let mut devices = match host.output_devices() {
        Ok(d) => d,
        Err(e) => panic!("Output devices Not Found: {}", e),
    };
    devices
        .find(|x| x.name().map(|y| y == *device_name).unwrap_or(false))
        .unwrap()
}

pub fn get_flushed_broadcast_queue<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
    queue: multiqueue::BroadcastReceiver<T>,
) -> multiqueue::BroadcastReceiver<T> {
    while queue.try_recv().is_ok() {}
    return queue;
}
