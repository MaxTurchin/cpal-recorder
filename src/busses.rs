use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};

use multiqueue::BroadcastSender;
use std::sync::mpsc::Receiver;

use std::marker::PhantomData;

#[derive(Debug)]
pub enum BusConfig {
    Mono,
    Stereo,
}

impl BusConfig {
    pub fn get_bus_config(nof_channels: &u8) -> BusConfig {
        // println!("nof_channels: {}", nof_channels);
        match nof_channels {
            1 => BusConfig::Mono,
            2 => BusConfig::Stereo,
            n => panic!("get_bus_config: Oh no! invalid nof channels: {}", n),
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
        txs: Vec<BroadcastSender<(u8, T)>>,
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
        println!("Broadcast stream started!");
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

    pub fn to_string(&self) -> String {
        return format!("Channels: {:?}", self.channel_ids);
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
        rx: Receiver<(u8, T)>,
    ) -> OutputBus<T> {
        let nof_channels = config.channels as u8;
        let ch_ids = channel_ids.clone();
        let stream = device
            .build_output_stream(
                &config,
                move |data, _: &_| {
                    playback_clb::<T>(data, &rx, &ch_ids);
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

    pub fn play_stream(&self) {
        println!("Playback stream started!");
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

    pub fn to_string(&self) -> String {
        return format!("Channels: {:?}", self.channel_ids);
    }
}

fn broadcast_clb<T: cpal::Sample>(
    data: &[T],
    txs: &Vec<BroadcastSender<(u8, T)>>,
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
                        tx.try_send((1 as u8, sample));
                        tx.try_send((2 as u8, sample));
                    }
                    BusConfig::Stereo => {
                        tx.try_send((cur_ch, sample));
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

fn playback_clb<T: cpal::Sample>(data: &mut [T], rx: &Receiver<(u8, T)>, out_channels: &Vec<u8>) {
    let mut ch_idx = 0;
    for sample in data {
        let (dest_ch, s_data) = match rx.recv().ok() {
            Some(t) => t,
            None => continue,
        };

        if dest_ch == out_channels[ch_idx] {
            ch_idx += 1;
            if ch_idx >= out_channels.len() {
                ch_idx = 0;
            }
            *sample = s_data;
        } else {
            ch_idx += 1;
            if ch_idx >= out_channels.len() {
                ch_idx = 0;
            }
        }
    }
}

pub fn err_fn(error: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", error);
}
