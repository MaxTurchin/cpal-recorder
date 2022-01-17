use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Host, SampleFormat, Stream, StreamConfig};

use multiqueue::{BroadcastSender, BroadcastReceiver};
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

    let (tx, rx) = multiqueue::broadcast_queue::<(u8, f32)>(2000);
    let (tx, rx) = multiqueue::broadcast_queue::<(u8, f32)>(2000);
    
    let track_rx1 = rx.clone(); //must add_streams to cloned rx.
    let track_rx2 = track_rx1.add_stream();
    let track_rx3 = track_rx1.add_stream();

    let in_bus1 = InputBus::<f32>::new(device, stream_config.clone(), vec![1 as u8], tx);
    // let in_bus2 = InputBus::<f32>::new(device, stream_config.clone(), vec![1 as u8], tx);

    let track1 = Track::new(1,"track".to_string(), stream_config.clone(), sample_format);
    let track2 = Track::new(1,"track1".to_string(), stream_config.clone(), sample_format);
    let track3 = Track::new(1,"track2".to_string(), stream_config.clone(), sample_format);

    in_bus1.stream.play();

    let (thread_tx1, term_tx1) = track1.record::<f32>();
    let (thread_tx2, term_tx2) = track2.record::<f32>();
    let (thread_tx3, term_tx3) = track3.record::<f32>();
    thread_tx1.send(track_rx1);
    thread_tx2.send(track_rx2);
    thread_tx3.send(track_rx3);


    std::thread::sleep(std::time::Duration::from_secs(10));

    term_tx1.send(());
    term_tx2.send(());
    term_tx3.send(());
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
                        break;
                    }
                    Err(_) => continue,
                }
            }
            continue;
        }
        channel_id += 1;
        if channel_id > *channel_ids.last().unwrap() {
            channel_id = 1;
        }
    }
}

fn write_thread<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
    writer: WavWriterHandle,
    thread_rx: std::sync::mpsc::Receiver<multiqueue::BroadcastReceiver<(u8, T)>>,
    term_rx: std::sync::mpsc::Receiver<()>
) {
    std::thread::spawn(move || {
        if let Ok(mut guard) = writer.try_lock() {
            if let Some(writer) = guard.as_mut() {
                
                let bus_rx: multiqueue::BroadcastReceiver<(u8, T)> = thread_rx.recv().unwrap();
                

                //Start reading from bus_rx and writing to file.
                loop {
                    //Tries to receive info from broadcast buffer.
                    match bus_rx.try_recv() {
                        Ok(s) => {
                            let sample: T = cpal::Sample::from(&s.1);
                            // writer.write_sample(sample).ok();
                            writer.write_sample(sample).ok();
                        }
                        Err(_) => (),
                    }

                    //Looks for signal to terminate thread.
                    match term_rx.try_recv() {
                        Ok(_) => break,
                        Err(_) => continue
                    }
                }
            }
        }
    });
}




pub struct InputBus<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> {
    device: Device,
    input_config: StreamConfig,
    channel_ids: Vec<u8>,
    track_ids: Vec<u8>,
    tx: BroadcastSender<(u8, T)>,
    pub stream: Stream
}

impl <T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> InputBus<T> {
    pub fn new(device: Device, config: StreamConfig, channel_ids: Vec<u8>, tx: BroadcastSender<(u8,T)>) -> InputBus<T>{
        
        let ch_ids = channel_ids.clone();
        let tx1 = tx.clone();

        let stream = device.build_input_stream(&config, move |data, _: &_| {
            broadcast_clb::<T>(data, &tx.clone(), &channel_ids.clone())},
            err_fn
        ).unwrap();

        InputBus::<T> {
            device: device,
            input_config: config,
            channel_ids: ch_ids,
            track_ids: Vec::<u8>::new(),
            tx: tx1,
            stream: stream
        }
    }

    pub fn add_track(&mut self, track_id: u8) {
        self.track_ids.push(track_id);

        let tx = self.tx.clone();
        let channel_ids = self.channel_ids.clone();

        self.stream = self.device.build_input_stream(&self.input_config, move |data, _: &_| {
            broadcast_clb::<T>(data, &tx.clone(), &channel_ids.clone())},
            err_fn
        ).unwrap();
    }
}



pub struct Track {
    id: u8,
    name: String,
    files: Vec<String>,
    wav_spec: hound::WavSpec
}


impl Track {
    pub fn new(id: u8, name: String, stream_config: StreamConfig, sample_format: SampleFormat) -> Track {
        
        let wav_spec = wav_spec_from_config(&stream_config, &sample_format);
        Track {
            id: id,
            name: name.clone(),
            files: vec![format!("{}_1.wav", name)],
            wav_spec: wav_spec
        }
    }

    pub fn add_file(&mut self) {
        let fname = format!("{}_{}.wav", self.name, self.files.len() + 1);
        self.files.push(fname);
    }

    pub fn record<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(&self) -> (std::sync::mpsc::Sender<multiqueue::BroadcastReceiver<(u8, T)>> ,std::sync::mpsc::Sender<()>) {
        let writer = hound::WavWriter::create(self.files.last().unwrap(), self.wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        
        let (thread_tx, thread_rx) = std::sync::mpsc::channel();
        let (term_tx, term_rx) = std::sync::mpsc::channel();
        write_thread(writer, thread_rx, term_rx);

        return (thread_tx, term_tx);
    }

}
