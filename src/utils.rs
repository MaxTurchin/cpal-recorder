use std::fs::File;
use std::io::BufWriter;
use std::sync::{
    Arc,
    Mutex
};
use cpal::{
    Stream,
    Device,
    StreamConfig,
    SampleFormat
};
use cpal::traits::{
    DeviceTrait,
    HostTrait
};
use ringbuf::{
    RingBuffer,
    Consumer,
    Producer
};


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
    for host_id in cpal::available_hosts(){
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
    println!("\tChannle Count: {}\n\tSample Rate: {}\n\tbuffer size: {:?}",
             config.channels, config.sample_rate.0, config.buffer_size);
}


pub fn wav_spec_from_config(config:   &StreamConfig,
                            sample_f: &SampleFormat) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels as _,
        sample_rate: config.sample_rate.0 as _,
        bits_per_sample: (sample_f.sample_size() * 8) as _,
        sample_format: sample_format(*sample_f),
    }
}


pub fn make_write_stream(input_config:  &StreamConfig,
                         input_device:  &Device,
                         sample_format: &SampleFormat,
                         writer:        &WavWriterHandle) -> Stream {

    let wav_writer = writer.clone();

    let input_stream: Stream = match sample_format {
        cpal::SampleFormat::F32 => input_device.build_input_stream(
            &input_config,
            move |data, _: &_| write_input_data::<f32, f32>(data, &wav_writer),
            err_fn,
        ).unwrap(),

        cpal::SampleFormat::I16 => input_device.build_input_stream(
            &input_config,
            move |data, _: &_| write_input_data::<i16, i16>(data, &wav_writer),
            err_fn,
        ).unwrap(),

        cpal::SampleFormat::U16 => input_device.build_input_stream(
            &input_config,
            move |data, _: &_| write_input_data::<u16, i16>(data, &wav_writer),
            err_fn
        ).unwrap()
    };
    return input_stream;
}


pub fn make_monitor_streams(input_config:  &StreamConfig,
                            output_config: &StreamConfig,
                            sample_format: &SampleFormat,
                            input_device:  &Device,
                            output_device: &Device) -> (Stream, Stream) {
    let latency = 300.0;
    let frames = (latency / 1_000.0) * (input_config.sample_rate.0 as f32);
    let mono_stereo = MonoStereo::channels_to_enum(input_config.channels);

    let (monitor_input, monitor_output): (Stream, Stream) = match sample_format {
        cpal::SampleFormat::F32 => {
            let latency_samples = (frames as f32) as usize * input_config.channels as usize;

            let buffer = RingBuffer::<f32>::new(latency_samples * 2);
            let (mut producer, mut consumer) = buffer.split();

            for _ in 0..latency_samples {
                producer.push(0.0).unwrap();
            }

            (
                input_device.build_input_stream(
                &input_config,
                move |data, _: &_| write_input_data_to_buf::<f32>(data, &mut producer, &mono_stereo),
                err_fn).unwrap(),

                output_device.build_output_stream(
                &output_config,
                move |data, _: &_| read_data_from_buf::<f32>(data, &mut consumer),
                err_fn).unwrap(),
            )

        },

        cpal::SampleFormat::I16 => {
            let latency_samples = (frames as i16) as usize * input_config.channels as usize;

            let buffer = RingBuffer::<i16>::new(latency_samples * 2);
            let (mut producer, mut consumer) = buffer.split();

            for _ in 0..latency_samples {
                producer.push(0).unwrap();
            }

            (
                input_device.build_input_stream(
                &input_config,
                move |data, _: &_| write_input_data_to_buf::<i16>(data, &mut producer, &mono_stereo),
                err_fn).unwrap(),

                output_device.build_output_stream(
                &output_config,
                move |data, _: &_| read_data_from_buf::<i16>(data, &mut consumer),
                err_fn).unwrap(),
            )
        },

        cpal::SampleFormat::U16 => {
            let latency_samples = (frames as i16) as usize * input_config.channels as usize;

            let buffer = RingBuffer::<u16>::new(latency_samples * 2);
            let (mut producer, mut consumer) = buffer.split();

            for _ in 0..latency_samples {
                producer.push(0).unwrap();
            }

            (
                input_device.build_input_stream(
                &input_config,
                move |data, _: &_| write_input_data_to_buf::<u16>(data, &mut producer, &mono_stereo),
                err_fn).unwrap(),

                output_device.build_output_stream(
                &output_config,
                move |data, _: &_| read_data_from_buf::<u16>(data, &mut consumer),
                err_fn).unwrap(),
            )
        }
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

//Used for write streams
fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
where
    T: cpal::Sample,
    U: cpal::Sample + hound::Sample,
{
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.as_mut() {
            for &sample in input.iter() {
                let sample: U = cpal::Sample::from(&sample);
                writer.write_sample(sample).ok();
            }
        }
    }
}


enum MonoStereo {
    MONO,
    STEREO,
    INVALID
}

impl MonoStereo {
    pub fn channels_to_enum(nof_channels: cpal::ChannelCount) -> MonoStereo {
        match nof_channels {
            1 => MonoStereo::MONO,
            2 => MonoStereo::STEREO,
            _ => MonoStereo::INVALID
        }
    }
}


//Used for monitor streams
fn write_input_data_to_buf<T>(data:     &[T],
                              producer: &mut Producer<T>,
                              conf:     &MonoStereo)
where
    T: cpal::Sample
{
    for &sample in data {
        producer.push(sample).ok();
        if let MonoStereo::MONO = conf {
            producer.push(sample).ok();
        }
    }
}
fn read_data_from_buf<T>(data: &mut [T], consumer: &mut Consumer<T>)
where
    T: cpal::Sample
{
    for sample in data {
        *sample = match consumer.pop() {
            Some(s) => s,
            None => continue
        };
    }
}
