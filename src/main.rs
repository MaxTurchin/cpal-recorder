use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, Stream, StreamConfig};

use multiqueue::{BroadcastReceiver, BroadcastSender};
use std::fs::File;
use std::io::BufWriter;
use std::sync::mpsc::{self, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() {
    println!("Hello, world!");
    let host = cpal::default_host();
    let device = host.default_input_device().unwrap();
    let stream_config = device.default_input_config().unwrap().config();

    let sample_format = device.default_input_config().unwrap().sample_format();

    let mut router = Router::<f32>::new(
        host,
        stream_config,
        "Analogue 1 + 2 (Focusrite Usb Audio)".to_string(),
        sample_format,
    );
    router.new_input_bus(vec![1 as u8]);
    router.new_input_bus(vec![2 as u8]);

    router.new_track("Tractor".to_string(), 0);
    router.new_track("Tractor1".to_string(), 1);
    router.new_track("Tractor2".to_string(), 0);

    router.record();
    thread::sleep(std::time::Duration::from_secs(10));
    router.stop_recording();
    println!("Done!");

    router.record();
    thread::sleep(std::time::Duration::from_secs(10));
    router.stop_recording();
    println!("Done!");
}

pub type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;

pub fn err_fn(error: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", error);
}

pub fn wav_spec_from_config(config: &StreamConfig, sample_f: &SampleFormat) -> hound::WavSpec {
    hound::WavSpec {
        channels: config.channels as _,
        sample_rate: config.sample_rate.0 as _,
        bits_per_sample: (sample_f.sample_size() * 8) as _,
        sample_format: sample_format(*sample_f),
    }
}

pub fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
    match format {
        cpal::SampleFormat::U16 => hound::SampleFormat::Int,
        cpal::SampleFormat::I16 => hound::SampleFormat::Int,
        cpal::SampleFormat::F32 => hound::SampleFormat::Float,
    }
}

fn broadcast_clb<T: cpal::Sample>(
    data: &[T],
    tx: &multiqueue::BroadcastSender<(u8, T)>,
    channel_ids: &Vec<u8>,
    nof_channels: &u8,
) {
    let mut channel_id: u8 = 1;
    for &sample in data {
        if channel_ids.contains(&channel_id) {
            loop {
                match tx.try_send((channel_id, sample)) {
                    Ok(_) => (),
                    Err(_) => continue,
                }
                match tx.try_send((channel_id, sample)) {
                    Ok(_) => {
                        channel_id += 1;
                        if channel_id > *nof_channels {
                            channel_id = 1;
                        }
                        break;
                    }
                    Err(_) => continue,
                }
            }
            continue;
        }

        channel_id += 1;
        if channel_id > *nof_channels {
            channel_id = 1;
        }
    }
}

fn write_thread<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
    writer: WavWriterHandle,
    thread_rx: std::sync::mpsc::Receiver<multiqueue::BroadcastReceiver<(u8, T)>>,
    term_rx: std::sync::mpsc::Receiver<()>,
) {
    thread::spawn(move || {
        if let Ok(mut guard) = writer.try_lock() {
            if let Some(writer) = guard.as_mut() {
                let bus_rx: multiqueue::BroadcastReceiver<(u8, T)> = thread_rx.recv().unwrap();
                println!("Received");
                //Start reading from bus_rx and writing to file.
                loop {
                    //Tries to receive info from broadcast buffer.
                    match bus_rx.try_recv() {
                        Ok(s) => {
                            let sample: T = cpal::Sample::from(&s.1);
                            writer.write_sample(sample).ok();
                        }
                        Err(e) => (),
                    }

                    //Looks for signal to terminate thread.
                    match term_rx.try_recv() {
                        Ok(_) => break,
                        Err(_) => continue,
                    }
                }
            }
        }
    });
}

pub struct InputBus<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> {
    id: u8,
    device: Device,
    input_config: StreamConfig,
    nof_channels: u8,
    channel_ids: Vec<u8>,
    track_ids: Vec<u8>,
    tx: BroadcastSender<(u8, T)>,
    pub stream: Stream,
}

impl<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> InputBus<T> {
    pub fn new(
        id: u8,
        device: Device,
        config: StreamConfig,
        channel_ids: Vec<u8>,
        tx: BroadcastSender<(u8, T)>,
    ) -> InputBus<T> {
        let ch_ids = channel_ids.clone();
        let tx1 = tx.clone();
        let nof_channels = config.channels as u8;

        let stream = device
            .build_input_stream(
                &config,
                move |data, _: &_| {
                    broadcast_clb::<T>(data, &tx.clone(), &channel_ids.clone(), &nof_channels)
                },
                err_fn,
            )
            .unwrap();

        InputBus::<T> {
            id: id,
            device: device,
            input_config: config,
            nof_channels: nof_channels,
            channel_ids: ch_ids,
            track_ids: Vec::<u8>::new(),
            tx: tx1,
            stream: stream,
        }
    }

    pub fn add_track(&mut self, track_id: u8) {
        self.track_ids.push(track_id);
    }
}

pub struct Track {
    id: u8,
    name: String,
    files: Vec<String>,
    wav_spec: hound::WavSpec,
}

impl Track {
    pub fn new(
        id: u8,
        name: String,
        stream_config: StreamConfig,
        sample_format: SampleFormat,
    ) -> Track {
        let wav_spec = wav_spec_from_config(&stream_config, &sample_format);
        Track {
            id: id,
            name: name.clone(),
            files: Vec::<String>::new(),
            wav_spec: wav_spec,
        }
    }

    pub fn record<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
        &mut self,
    ) -> (
        std::sync::mpsc::Sender<multiqueue::BroadcastReceiver<(u8, T)>>,
        std::sync::mpsc::Sender<()>,
    ) {
        self.add_file();

        let writer = hound::WavWriter::create(self.files.last().unwrap(), self.wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        let (thread_tx, thread_rx) = std::sync::mpsc::channel();
        let (term_tx, term_rx) = std::sync::mpsc::channel();
        write_thread(writer, thread_rx, term_rx);

        return (thread_tx, term_tx);
    }

    fn add_file(&mut self) {
        let fname = format!("{}_{}.wav", self.name, self.files.len() + 1);
        self.files.push(fname);
    }
}

pub struct Router<T: 'static + cpal::Sample + hound::Sample + Send + Sync> {
    host: Host,
    input_config: StreamConfig,
    //output_config: StreamConfig
    device_name: String,
    sample_format: SampleFormat,
    tracks: Vec<Track>,
    input_buses: Vec<(BroadcastReceiver<(u8, T)>, InputBus<T>)>, //(bus_rx, input_bus)
    //output_buses: Vec<OutputBus>
    track_term_txs: Vec<std::sync::mpsc::Sender<()>>,
}

impl<T: 'static + cpal::Sample + hound::Sample + Send + Sync> Router<T> {
    pub fn new(
        host: Host,
        in_config: StreamConfig,
        device_name: String,
        sample_format: SampleFormat,
    ) -> Router<T> {
        Router {
            host: host,
            input_config: in_config,
            device_name: device_name,
            sample_format: sample_format,
            tracks: Vec::<Track>::new(),
            input_buses: Vec::<(BroadcastReceiver<(u8, T)>, InputBus<T>)>::new(),
            track_term_txs: Vec::<std::sync::mpsc::Sender<()>>::new(),
        }
    }

    pub fn new_input_bus(&mut self, channel_ids: Vec<u8>) {
        let bus_id = self.input_buses.len() as u8;
        let device = self.get_device();

        let (bus_tx, bus_rx) = multiqueue::broadcast_queue::<(u8, T)>(2000);
        let in_bus = InputBus::<T>::new(
            bus_id,
            device,
            self.input_config.clone(),
            channel_ids,
            bus_tx,
        );

        self.input_buses.push((bus_rx, in_bus));
    }

    //pub fn new_output_bus()

    pub fn new_track(&mut self, track_name: String, bus_id: u8) {
        let track_id = self.tracks.len() as u8;
        let track = Track::new(
            track_id,
            track_name,
            self.input_config.clone(),
            self.sample_format,
        );

        self.input_buses[bus_id as usize].1.add_track(track_id);
        self.tracks.push(track);
    }

    pub fn record(&mut self) {
        for input_bus in self.input_buses.iter() {
            let track_ids = input_bus.1.track_ids.clone();
            let mut bus_rx = Box::new(input_bus.0.clone());
            for track_id in track_ids.iter() {
                let (thread_tx, term_tx) = self.tracks[*track_id as usize].record::<T>();
                self.track_term_txs.push(term_tx);

                let next_rx = bus_rx.clone();
                thread_tx.send(*next_rx);
                bus_rx = Box::new(bus_rx.add_stream());
            }
            input_bus.1.stream.play();
        }
    }

    pub fn stop_recording(&mut self) {
        for term_tx in &self.track_term_txs {
            term_tx.send(());
            println!("Terminated");
        }
        self.track_term_txs.clear();
    }

    fn get_device(&self) -> Device {
        let mut devices = match self.host.input_devices() {
            Ok(d) => d,
            Err(e) => panic!("Devices Not Found: {}", e),
        };
        devices
            .find(|x| x.name().map(|y| y == self.device_name).unwrap_or(false))
            .unwrap()
    }
}
