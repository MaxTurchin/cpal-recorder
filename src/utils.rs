use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use ringbuf::{Consumer, Producer, RingBuffer};
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};

use num_traits;

pub type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

pub fn show_hosts() {
    let host_ids = cpal::available_hosts();

    println!("Hosts:");
    for host in host_ids {
        println!("\t{}", host.name());
    }
    println!();
}

pub fn show_devices() {
    for host_id in cpal::available_hosts() {
        let host = cpal::host_from_id(host_id).unwrap();

        let input_devices = host.input_devices().unwrap();
        let output_devices = host.output_devices().unwrap();

        println!("Host: {}", host_id.name());
        println!("Input Devices:");
        for (index, device) in input_devices.enumerate() {
            println!("\t{}.{}", index + 1, device.name().unwrap());
        }

        println!("Output Devices:");
        for (index, device) in output_devices.enumerate() {
            println!("\t{}.{}", index + 1, device.name().unwrap());
        }
        println!();
    }
}

pub fn show_config(config: &StreamConfig) {
    println!(
        "\tChannle Count: {}\n\tSample Rate: {}\n\tbuffer size: {:?}",
        config.channels, config.sample_rate.0, config.buffer_size
    );
}

pub fn wav_spec_from_config(config: &StreamConfig, sample_f: &SampleFormat) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels as _,
        sample_rate: config.sample_rate.0 as _,
        bits_per_sample: (sample_f.sample_size() * 8) as _,
        sample_format: sample_format(*sample_f),
    }
}

fn get_write_stream<T, U>(
    device: &Device,
    device_conf: &StreamConfig,
    writer: &WavWriterHandle,
    write_conf: ReadWriteConfig,
) -> Stream
where
    T: num_traits::Num + cpal::Sample,
    U: num_traits::Num + cpal::Sample + hound::Sample,
{
    let writer = writer.clone();
    let mut write_conf = write_conf;
    return device
        .build_input_stream(
            &device_conf,
            move |data, _: &_| write_input_data::<T, U>(data, &writer, &mut write_conf),
            err_fn,
        )
        .unwrap();
}

pub fn make_write_stream<T, U>(
    input_config: &StreamConfig,
    input_device: &Device,
    mono_stereo: &MonoStereo,
    channels: &Vec<u8>,
    sample_format: &SampleFormat,
    writer: &WavWriterHandle,
) -> Stream {
    let write_conf = ReadWriteConfig {
        channel_ids: channels.clone(),
        mono_stereo: mono_stereo.clone(),
        nof_channels: input_config.channels as u8,
        sample_index: Box::<u8>::new(1),
    };

    return match sample_format {
        SampleFormat::F32 => {
            get_write_stream::<f32, f32>(input_device, input_config, writer, write_conf)
        }
        SampleFormat::I16 => {
            get_write_stream::<i16, i16>(input_device, input_config, writer, write_conf)
        }
        SampleFormat::U16 => {
            get_write_stream::<u16, i16>(input_device, input_config, writer, write_conf)
        }
    };
}

fn get_buf_input_stream<T>(
    device: &Device,
    device_conf: &StreamConfig,
    write_conf: ReadWriteConfig,
    producer: Producer<(u8, T)>,
) -> Stream
where
    T: num_traits::Num,
    T: cpal::Sample,
    T: std::marker::Send,
    T: 'static,
{
    let mut write_conf = write_conf;
    let mut producer = producer;
    return device
        .build_input_stream(
            &device_conf,
            move |data, _: &_| write_input_data_to_buf::<T>(data, &mut producer, &mut write_conf),
            err_fn,
        )
        .unwrap();
}

fn get_buf_output_stream<T>(
    output: &Device,
    output_conf: &StreamConfig,
    mut read_conf: ReadWriteConfig,
    consumer: Consumer<(u8, T)>,
) -> Stream
where
    T: num_traits::Num,
    T: cpal::Sample,
    T: std::marker::Send,
    T: 'static,
{
    let mut consumer = consumer;
    return output
        .build_output_stream(
            &output_conf,
            move |data, _: &_| read_data_from_buf::<T>(data, &mut consumer, &mut read_conf),
            err_fn,
        )
        .unwrap();
}

fn get_monitor_ringbuf<T, U>(latency_samples: usize) -> (Producer<(T, U)>, Consumer<(T, U)>)
where
    T: num_traits::Num,
    U: num_traits::Num,
{
    let buff = RingBuffer::<(T, U)>::new(latency_samples * 2);
    let (mut producer, consumer) = buff.split();

    for _ in 0..latency_samples {
        producer.push((T::zero(), U::zero()));
    }

    return (producer, consumer);
}

fn get_monitor_streams<T>(
    input_device: &Device,
    output_device: &Device,
    input_config: &StreamConfig,
    output_config: &StreamConfig,
    write_config: ReadWriteConfig,
    read_config: ReadWriteConfig,
) -> (Stream, Stream)
where
    T: num_traits::Num,
    T: cpal::Sample,
    T: std::marker::Send,
    T: 'static,
{
    let latency = 50.0;
    let frames = (latency / 1_000.0) * (input_config.sample_rate.0 as f32);

    let nof_samples = frames as usize * input_config.channels as usize;
    let latency_samples = nof_samples * std::mem::size_of::<(u8, T)>();

    let (producer, consumer) = get_monitor_ringbuf::<u8, T>(latency_samples);
    (
        get_buf_input_stream::<T>(input_device, input_config, write_config, producer),
        get_buf_output_stream::<T>(output_device, output_config, read_config, consumer),
    )
}

pub fn make_monitor_streams(
    input_config: &StreamConfig,
    output_config: &StreamConfig,
    sample_format: &SampleFormat,
    input_device: &Device,
    output_device: &Device,
    mono_stereo: &MonoStereo,
    input_channels: &Vec<u8>,
    output_channels: &Vec<u8>,
) -> (Stream, Stream) {
    let write_conf = ReadWriteConfig {
        channel_ids: input_channels.clone(),
        mono_stereo: mono_stereo.clone(),
        nof_channels: input_config.channels as u8,
        sample_index: Box::<u8>::new(1),
    };
    let read_conf = ReadWriteConfig {
        channel_ids: output_channels.clone(),
        mono_stereo: mono_stereo.clone(),
        nof_channels: output_config.channels as u8,
        sample_index: Box::<u8>::new(1),
    };
    let (monitor_input, monitor_output) = match sample_format {
        cpal::SampleFormat::F32 => get_monitor_streams::<f32>(
            input_device,
            output_device,
            input_config,
            output_config,
            write_conf,
            read_conf,
        ),
        cpal::SampleFormat::I16 => get_monitor_streams::<i16>(
            input_device,
            output_device,
            input_config,
            output_config,
            write_conf,
            read_conf,
        ),
        cpal::SampleFormat::U16 => get_monitor_streams::<u16>(
            input_device,
            output_device,
            input_config,
            output_config,
            write_conf,
            read_conf,
        ),
    };
    return (monitor_input, monitor_output);
}

fn err_fn(error: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", error);
}

fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    match format {
        cpal::SampleFormat::U16 => hound::SampleFormat::Int,
        cpal::SampleFormat::I16 => hound::SampleFormat::Int,
        cpal::SampleFormat::F32 => hound::SampleFormat::Float,
    }
}

pub enum MonoStereo {
    MONO,
    STEREO,
    INVALID,
}

impl Clone for MonoStereo {
    fn clone(&self) -> MonoStereo {
        match self {
            MonoStereo::MONO => MonoStereo::MONO,
            MonoStereo::STEREO => MonoStereo::STEREO,
            MonoStereo::INVALID => MonoStereo::INVALID,
        }
    }
}

// impl MonoStereo {
//     pub fn channels_to_enum(nof_channels: cpal::ChannelCount) -> MonoStereo {
//         match nof_channels {
//             1 => MonoStereo::MONO,
//             2 => MonoStereo::STEREO,
//             _ => MonoStereo::INVALID,
//         }
//     }
// }

struct ReadWriteConfig {
    mono_stereo: MonoStereo,
    channel_ids: Vec<u8>,
    nof_channels: u8,
    sample_index: Box<u8>,
}

impl ReadWriteConfig {
    //loops over number of channels starting from 1 to nof_channels
    fn cnt_increment(&mut self) {
        *self.sample_index += 1;
        if *self.sample_index > self.nof_channels as u8 {
            *self.sample_index = 1;
        }
    }
}

//Used for write streams
fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle, conf: &mut ReadWriteConfig)
where
    T: cpal::Sample,
    U: cpal::Sample + hound::Sample,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let cnt = *conf.sample_index;

                if conf.channel_ids.contains(&cnt) {
                    let sample: U = cpal::Sample::from(&sample);
                    writer.write_sample(sample).ok();

                    if let MonoStereo::MONO = conf.mono_stereo {
                        writer.write_sample(sample).ok();
                    }
                }
                conf.cnt_increment();
            }
        }
    }
}

//Used for monitor streams
fn write_input_data_to_buf<T>(
    data: &[T],
    producer: &mut Producer<(u8, T)>,
    conf: &mut ReadWriteConfig,
) where
    T: cpal::Sample,
{
    for &sample in data {
        if conf.channel_ids.contains(&*conf.sample_index) {
            producer.push((*conf.sample_index, sample)).ok();
            conf.cnt_increment();

            if let MonoStereo::MONO = conf.mono_stereo {
                producer.push((*conf.sample_index, sample)).ok();
            }
        } else {
            conf.cnt_increment();
        }
    }
}

fn read_data_from_buf<T>(
    data: &mut [T],
    consumer: &mut Consumer<(u8, T)>,
    conf: &mut ReadWriteConfig,
) where
    T: cpal::Sample,
{
    for sample in data {
        *sample = match consumer.pop() {
            Some(s) => {
                if s.0 == *conf.sample_index {
                    conf.cnt_increment();
                    s.1
                } else {
                    continue;
                }
            }
            None => continue,
        };
    }
}
