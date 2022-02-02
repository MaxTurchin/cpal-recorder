use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host, SampleFormat, StreamConfig};
use multiqueue::BroadcastReceiver;

use std::sync::mpsc::{self};
use std::sync::mpsc::{Receiver, Sender};

use std::io::Error;
use std::thread;

use crate::busses::{BusConfig, InputBus, OutputBus};
use crate::tracks::Track;
use crate::utils::{
    get_flushed_broadcast_queue, get_flushed_mpsc_queue, get_input_device_by_name,
    get_output_device_by_name,
};

struct RouteMap {
    routes: Vec<(u8, u8, Vec<u8>)>, // (input bus, output bus, track_list)
}

impl RouteMap {
    pub fn new() -> RouteMap {
        RouteMap {
            routes: Vec::<(u8, u8, Vec<u8>)>::new(),
        }
    }

    pub fn add_route(&mut self, in_bus_id: &u8, out_bus_id: &u8) {
        self.routes
            .push((*in_bus_id, *out_bus_id, Vec::<u8>::new()))
    }

    pub fn add_track_to_route(&mut self, in_id: &u8, out_id: &u8, track_id: &u8) {
        for route in self.routes.iter_mut() {
            let (input, output, track_ids) = (route.0, route.1, route.2.clone());
            if input == *in_id && output == *out_id && !track_ids.contains(track_id) {
                route.2.push(*track_id);
            }
        }
    }

    pub fn get_track_busses(&self, track_id: &u8) -> Option<(u8, u8)> {
        // (in_bus, out_bus)
        for route in self.routes.iter() {
            let tracks = &route.2;
            if tracks.contains(track_id) {
                return Some((route.0, route.1));
            }
        }
        return None;
    }

    pub fn get_route_track_ids(&self, in_id: &u8, out_id: &u8) -> Option<Vec<u8>> {
        for route in self.routes.iter() {
            let (input, output) = (route.0, route.1);
            if input == *in_id && output == *out_id {
                return Some(route.2.clone());
            }
        }
        return None;
    }
}

struct MonitorLink<T> {
    out_bus_id: u8,
    tx_to_bus: Sender<(u8, T)>,
    pub rxs_from_monitors: Vec<Receiver<(u8, T)>>,
}

impl<T> MonitorLink<T> {
    pub fn as_tup(self) -> (u8, Sender<(u8, T)>, Vec<Receiver<(u8, T)>>) {
        (self.out_bus_id, self.tx_to_bus, self.rxs_from_monitors)
    }
}

pub struct RouteConfig {
    pub host: Host,
    pub in_config: StreamConfig,
    pub out_config: StreamConfig,
    pub in_device: String,
    pub out_device: String,
    pub sample_format: SampleFormat,
}

pub struct Router<T: 'static + cpal::Sample + hound::Sample + Send + Sync> {
    pub config: RouteConfig,
    tracks: Vec<Track>,
    input_busses: Vec<(
        BroadcastReceiver<(u8, T)>,
        BroadcastReceiver<(u8, T)>,
        InputBus<T>,
    )>, // (record_rx, monitor_rx, input_bus)
    output_busses: Vec<(Sender<(u8, T)>, OutputBus<T>)>, //(bus_tx, output_bus)
    routes: RouteMap,
    monitor_txs: Vec<Sender<()>>,
}

impl<T: 'static + cpal::Sample + hound::Sample + Send + Sync> Router<T> {
    pub fn new(
        host: Host,
        in_config: StreamConfig,
        out_config: StreamConfig,
        in_device_name: String,
        out_device_name: String,
        sample_format: SampleFormat,
    ) -> Router<T> {
        Router {
            config: RouteConfig {
                host: host,
                in_config: in_config,
                out_config: out_config,
                in_device: in_device_name,
                out_device: out_device_name,
                sample_format: sample_format,
            },
            tracks: Vec::<Track>::new(),
            input_busses: Vec::<(
                BroadcastReceiver<(u8, T)>,
                BroadcastReceiver<(u8, T)>,
                InputBus<T>,
            )>::new(), //(Rx for recording, Rx for monitoring, InputBus)
            output_busses: Vec::<(Sender<(u8, T)>, OutputBus<T>)>::new(), //(Sender for sending samples, OutputBus)
            routes: RouteMap::new(),
            monitor_txs: Vec::<Sender<()>>::new(),
        }
    }

    pub fn new_input_bus(&mut self, channel_ids: Vec<u8>) -> u8 {
        let bus_id = self.input_busses.len() as u8;
        let device = get_input_device_by_name(&self.config.host, &self.config.in_device);

        let (bus_rec_tx, bus_rec_rx) = multiqueue::broadcast_queue::<(u8, T)>(1_000_000);
        let (bus_mon_tx, bus_mon_rx) = multiqueue::broadcast_queue::<(u8, T)>(1_000_000);
        let txs = vec![bus_rec_tx, bus_mon_tx];

        let bus_conf = BusConfig::get_bus_config(&(channel_ids.len() as u8));

        let in_bus = InputBus::<T>::new(
            bus_id,
            device,
            self.config.in_config.clone(),
            bus_conf,
            channel_ids,
            txs,
        );

        in_bus.play_stream();
        self.input_busses
            .push((bus_rec_rx.clone(), bus_mon_rx.clone(), in_bus));
        (self.input_busses.len() - 1) as u8
    }

    pub fn new_output_bus(&mut self, channel_ids: Vec<u8>) -> u8 {
        let bus_id = self.output_busses.len() as u8;
        let device = get_output_device_by_name(&self.config.host, &self.config.out_device);

        let (bus_tx, bus_rx) = mpsc::channel::<(u8, T)>();
        let out_bus = OutputBus::<T>::new(
            bus_id,
            device,
            self.config.out_config.clone(),
            channel_ids,
            bus_rx,
        );

        out_bus.play_stream();
        self.output_busses.push((bus_tx, out_bus));
        (self.output_busses.len() - 1) as u8
    }

    pub fn new_track(&mut self, track_name: String, in_bus_id: u8, out_bus_id: u8) {
        let track_id = self.tracks.len() as u8;
        println!("New track id: {}", track_id);
        let track = Track::new(
            track_id,
            track_name,
            self.config.in_config.clone(),
            self.config.sample_format,
        );

        self.input_busses[in_bus_id as usize].2.add_track(track_id);
        self.output_busses[out_bus_id as usize]
            .1
            .add_track(track_id);

        match self.routes.get_route_track_ids(&in_bus_id, &out_bus_id) {
            Some(_) => self
                .routes
                .add_track_to_route(&in_bus_id, &out_bus_id, &track_id),
            None => {
                self.routes.add_route(&in_bus_id, &out_bus_id);
                self.routes
                    .add_track_to_route(&in_bus_id, &out_bus_id, &track_id);
            }
        }
        self.tracks.push(track);
    }

    pub fn record(&mut self) {
        for input_bus in self.input_busses.iter_mut() {
            let track_ids = input_bus.2.get_track_ids();
            let mut bus_rx = Box::new(input_bus.0.clone());

            for track_id in track_ids.iter() {
                if self.tracks[*track_id as usize].is_rec_armed() {
                    let thread_tx = self.tracks[*track_id as usize].record::<T>();

                    thread_tx.send(*bus_rx.clone());
                    bus_rx = Box::new(bus_rx.add_stream());
                }
            }
        }
    }

    pub fn stop_recording(&mut self) {
        for input_bus in self.input_busses.iter() {
            let track_ids = input_bus.2.get_track_ids();
            for track_id in track_ids.iter() {
                self.tracks[*track_id as usize].stop_recording();
                println!("Terminated Recording (Track {})", track_id);
            }
        }
    }

    pub fn monitor(&mut self) {
        let mut links = Vec::<MonitorLink<T>>::new();

        for out in self.output_busses.iter_mut() {
            let (out_bus_id, out_bus_channels) = (out.1.get_id(), out.1.get_channel_ids());

            links.push(MonitorLink::<T> {
                out_bus_id: out_bus_id,
                tx_to_bus: out.0.clone(),
                rxs_from_monitors: Vec::<Receiver<(u8, T)>>::new(),
            });

            for input in self.input_busses.iter() {
                let in_bus_id = input.2.get_id();

                let mut in_bus_rx = Box::new(get_flushed_broadcast_queue(input.1.clone()));

                let tracks = match self.routes.get_route_track_ids(&in_bus_id, &out_bus_id) {
                    Some(t) => t,
                    None => Vec::new(),
                };

                // println!("Run monitor streams");
                for track_id in tracks.iter() {
                    if self.tracks[*track_id as usize].is_monitored() {
                        let (monitor_tx, monitor_rx) = self.tracks[*track_id as usize]
                            .start_monitor::<T>(out_bus_channels.clone());
                        let links_len = links.len();

                        get_flushed_mpsc_queue(&monitor_rx); //flush queue
                        links[links_len - 1].rxs_from_monitors.push(monitor_rx);

                        monitor_tx.send(*in_bus_rx.clone());
                        in_bus_rx = Box::new(in_bus_rx.add_stream());
                    } else if !self.tracks[*track_id as usize].is_rec_armed() {
                        let playback_rx: Receiver<(u8, T)> = match self.tracks[*track_id as usize]
                            .start_playback(out_bus_channels.clone())
                        {
                            Some(rx) => rx,
                            None => continue,
                        };
                        let links_len = links.len();
                        links[links_len - 1].rxs_from_monitors.push(playback_rx);
                    }
                }
            }
        }
        self._run_monitor_out_streams(links);
    }

    fn _run_monitor_out_streams(&mut self, mut links: Vec<MonitorLink<T>>) {
        while let Ok(link) = links.pop().ok_or("") {
            println!("pop");
            let (out_id, out_tx, monitor_rxs) = link.as_tup();
            // println!("monitor_rxs      : {}", monitor_rxs.len());
            let (thread_tx, thread_rx) = mpsc::channel::<Vec<Receiver<(u8, T)>>>();
            let (term_tx, term_rx) = mpsc::channel();
            let out_channels = self.output_busses[out_id as usize].1.get_channel_ids();

            mix_thread(thread_rx, term_rx, out_tx, out_channels);
            thread_tx.send(monitor_rxs);
            self.monitor_txs.push(term_tx);
        }
    }

    pub fn set_monitor(&mut self, track_id: u8, state: bool) {
        self.tracks[track_id as usize].set_monitor(state);
    }

    pub fn set_recording(&mut self, track_id: u8, state: bool) {
        self.tracks[track_id as usize].set_rec(state);
    }

    pub fn stop_monitor(&mut self) {
        //terminates mix monitor threads
        while let Ok(term_tx) = self.monitor_txs.pop().ok_or(err_fn) {
            println!("Terminating mix_thread");
            term_tx.send(());
        }
        //terminates track monitor threads
        for input_bus in self.input_busses.iter() {
            let track_ids = input_bus.2.get_track_ids();
            for track_id in track_ids.iter() {
                self.tracks[*track_id as usize].stop_monitor();
                println!("Terminated Monitor (Track {})", track_id);
            }
        }
    }

    pub fn get_io_channels(&self) -> (Vec<u8>, Vec<u8>) {
        //(input_channel_ids, output_channel_ids)
        (
            (1..=self.config.in_config.channels as u8).collect(),
            (1..=self.config.out_config.channels as u8).collect(),
        )
    }

    pub fn get_tracks(&self) -> &Vec<Track> {
        &self.tracks
    }
}

fn mix_thread<T: 'static + cpal::Sample + Send>(
    thread_rx: Receiver<Vec<Receiver<(u8, T)>>>,
    term_rx: Receiver<()>,
    out_tx: Sender<(u8, T)>,
    out_channels: Vec<u8>,
) {
    println!("Mix Thread spawned!");
    thread::spawn(move || {
        //TODO: support i16 and u16 sample formats

        let mut track_rxs = match thread_rx.recv() {
            Ok(tx) => tx,
            Err(e) => panic!("mix_thread: Oh no! {}", e),
        };

        loop {
            for ch in out_channels.iter() {
                let mut samples_avg = 0.0;
                for idx in 0..(track_rxs.len()) {
                    loop {
                        let (dest_ch, sample) = match track_rxs[idx].recv() {
                            Ok(t) => t,
                            Err(_) => {
                                track_rxs.remove(idx);
                                break;
                            }
                        };

                        if dest_ch != *ch || sample.to_f32().is_nan() {
                            continue;
                        }
                        samples_avg += sample.to_f32();
                        break;
                    }
                }
                samples_avg = samples_avg as f32 / track_rxs.len() as f32;
                if samples_avg.is_nan() {
                    continue;
                }
                out_tx.send((*ch, cpal::Sample::from(&samples_avg)));
            }

            if let Ok(_) = term_rx.try_recv() {
                break;
            }

            match thread_rx.try_recv() {
                Ok(mut rxs) => track_rxs.append(&mut rxs),
                Err(_) => continue,
            }
        }
    });
}

pub fn err_fn(error: Error) {
    eprintln!("an error occurred on stream: {}", error);
}
