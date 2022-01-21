use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};

use multiqueue::BroadcastSender;
use std::sync::mpsc::Receiver;

use std::marker::PhantomData;

pub enum BusConfig {
    Mono,
    Stereo,
}

impl BusConfig {
    pub fn get_bus_config(nof_channels: &u8) -> BusConfig {
        println!("nof_channels: {}", nof_channels);
        match nof_channels {
            1 => BusConfig::Mono,
            2 => BusConfig::Stereo,
            n => panic!("get_bus_config: Oh no invalid nof channels: {}", n),
        }
    }
}

pub struct InputBus<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> {
    id: u8,
    track_ids: Vec<u8>,
    channel_ids: Vec<u8>,
    pub stream: Stream,
    _type: PhantomData<T>,
}

impl<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> InputBus<T> {
    pub fn new(
        id: u8,
        device: Device,
        stream_config: StreamConfig,
        bus_config: BusConfig,
        channel_ids: Vec<u8>,
        txs: Vec<BroadcastSender<T>>,
    ) -> InputBus<T> {
        let nof_channels = stream_config.channels as u8;
        let ch_ids = channel_ids.clone();

        let stream = device
            .build_input_stream(
                &stream_config,
                move |data, _: &_| {
                    broadcast_clb::<T>(data, &txs.clone(), &ch_ids, &nof_channels, &bus_config)
                },
                err_fn,
            )
            .unwrap();

        InputBus::<T> {
            id: id,
            track_ids: Vec::<u8>::new(),
            channel_ids: channel_ids,
            stream: stream,
            _type: PhantomData::<T>,
        }
    }

    pub fn add_track(&mut self, track_id: u8) {
        self.track_ids.push(track_id);
    }

    pub fn play_stream(&self) {
        self.stream.play();
    }

    pub fn get_id(&self) -> u8 {
        self.id.clone()
    }

    pub fn get_track_ids(&self) -> Vec<u8> {
        self.track_ids.clone()
    }

    pub fn get_channel_ids(&self) -> Vec<u8> {
        self.channel_ids.clone()
    }
}

pub struct OutputBus<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> {
    id: u8,
    track_ids: Vec<u8>,
    channel_ids: Vec<u8>,
    pub stream: Stream,
    _type: PhantomData<T>,
}

impl<T: 'static + std::clone::Clone + cpal::Sample + Send + Sync> OutputBus<T> {
    pub fn new(
        id: u8,
        device: Device,
        config: StreamConfig,
        channel_ids: Vec<u8>,
        rx: Receiver<T>,
    ) -> OutputBus<T> {
        let nof_channels = config.channels as u8;
        let ch_ids = channel_ids.clone();
        let stream = device
            .build_output_stream(
                &config,
                move |data, _: &_| {
                    playback_clb::<T>(data, &rx);
                },
                err_fn,
            )
            .unwrap();

        OutputBus::<T> {
            id: id,
            track_ids: Vec::<u8>::new(),
            channel_ids: channel_ids,
            stream: stream,
            _type: PhantomData::<T>,
        }
    }

    pub fn add_track(&mut self, track_id: u8) {
        self.track_ids.push(track_id);
    }

    pub fn play_stream(&mut self) {
        self.stream.play();
    }

    pub fn get_id(&self) -> u8 {
        self.id.clone()
    }

    pub fn get_track_ids(&self) -> Vec<u8> {
        self.track_ids.clone()
    }

    pub fn get_channel_ids(&self) -> Vec<u8> {
        self.channel_ids.clone()
    }
}

fn broadcast_clb<T: cpal::Sample>(
    data: &[T],
    txs: &Vec<BroadcastSender<T>>,
    in_chs: &Vec<u8>,
    nof_chs: &u8,
    bus_config: &BusConfig,
) {
    let mut cur_ch = 1;
    for &sample in data {
        if in_chs.contains(&cur_ch) {
            for tx in txs {
                match bus_config {
                    BusConfig::Mono => {
                        tx.try_send(sample);
                        tx.try_send(sample);
                    }
                    BusConfig::Stereo => {
                        tx.try_send(sample);
                    }
                };
            }
            cur_ch += 1;
            if cur_ch > *nof_chs {
                cur_ch = 1;
            }
        } else {
            cur_ch += 1;
            if cur_ch > *nof_chs {
                cur_ch = 1;
            }
        }
    }
}

fn playback_clb<T: cpal::Sample>(data: &mut [T], rx: &Receiver<T>) {
    let mut channel_id: u8 = 1;
    for sample in data {
        *sample = match rx.recv().ok() {
            Some(s) => s,
            None => continue,
        };
    }
}

pub fn err_fn(error: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", error);
}
