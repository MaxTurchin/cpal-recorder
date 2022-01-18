use cpal::{SampleFormat, StreamConfig};
use multiqueue::BroadcastReceiver;

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use hound::{WavSpec, WavWriter};
use std::fs::File;
use std::io::BufWriter;

pub struct Track {
    id: u8,
    name: String,
    files: Vec<String>,
    wav_spec: WavSpec,
    term_tx: Vec<Sender<()>>,
    rec: bool,
    monitor: bool,
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
            term_tx: Vec::<Sender<()>>::new(),
            rec: false,
            monitor: false,
        }
    }

    pub fn record<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
        &mut self,
    ) -> Sender<BroadcastReceiver<T>> {
        self.add_file();

        let writer = WavWriter::create(self.files.last().unwrap(), self.wav_spec).unwrap();
        let writer = Arc::new(Mutex::new(Some(writer)));

        let (thread_tx, thread_rx) = std::sync::mpsc::channel::<BroadcastReceiver<T>>();
        let (term_tx, term_rx) = std::sync::mpsc::channel();
        write_thread(writer, thread_rx, term_rx);
        self.term_tx.push(term_tx);

        thread_tx
    }

    pub fn start_monitor<T: 'static + cpal::Sample + Send + Sync>(
        &mut self,
        out_chs: Vec<u8>,
    ) -> (
        Sender<BroadcastReceiver<T>>, // tx for sending bus_rx
        Receiver<T>,                  //rx for receiving Samples
    ) {
        let (thread_tx, thread_rx) = std::sync::mpsc::channel();
        let (term_tx, term_rx) = std::sync::mpsc::channel();
        let (monitor_tx, monitor_rx) = std::sync::mpsc::channel::<T>();

        monitor_thread(thread_rx, monitor_tx, term_rx, out_chs);
        self.term_tx.push(term_tx);

        (thread_tx, monitor_rx)
    }

    //Must be called after stop_monitor
    pub fn stop_recording(&mut self) {
        self.stop_thread();
    }

    pub fn stop_monitor(&mut self) {
        self.stop_thread();
    }

    pub fn arm_rec(&mut self) {
        self.rec = true;
    }

    pub fn arm_monitor(&mut self) {
        self.monitor = true;
    }

    pub fn is_rec_armed(&self) -> bool {
        self.rec
    }

    pub fn is_monitored(&self) -> bool {
        self.monitor
    }

    fn stop_thread(&mut self) {
        let tx = match self.term_tx.pop() {
            Some(t) => t,
            None => return,
        };
        tx.send(());
    }

    fn add_file(&mut self) {
        let fname = format!("{}_{}.wav", self.name, self.files.len() + 1);
        self.files.push(fname);
    }
}

fn write_thread<T: 'static + cpal::Sample + hound::Sample + Send + Sync>(
    writer: WavWriterHandle,
    thread_rx: Receiver<BroadcastReceiver<T>>,
    term_rx: Receiver<()>,
) {
    thread::spawn(move || {
        if let Ok(mut guard) = writer.try_lock() {
            if let Some(writer) = guard.as_mut() {
                let bus_rx: BroadcastReceiver<T> = match thread_rx.recv() {
                    Ok(rx) => rx,
                    Err(e) => panic!("write_thread: Oh no! {}", e),
                };
                println!("Received");

                //Start reading from bus_rx and writing to file.
                loop {
                    //Tries to receive info from broadcast buffer.
                    if let Ok(s) = bus_rx.try_recv() {
                        let sample: T = cpal::Sample::from(&s);
                        writer.write_sample(sample).ok();
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

fn monitor_thread<T: 'static + cpal::Sample + Send + Sync>(
    thread_rx: Receiver<BroadcastReceiver<T>>,
    monitor_tx: Sender<T>,
    term_rx: Receiver<()>,
    out_chs: Vec<u8>,
) {
    thread::spawn(move || {
        let bus_rx: BroadcastReceiver<T> = thread_rx.recv().unwrap();
        println!("Received");
        loop {
            let sample = match bus_rx.try_recv() {
                Ok(s) => s,
                Err(_) => continue,
            };
            monitor_tx.send(sample);
            match term_rx.try_recv() {
                Ok(_) => break,
                Err(_) => continue,
            }
        }
    });
}

pub type WavWriterHandle = Arc<Mutex<Option<WavWriter<BufWriter<File>>>>>;

pub fn wav_spec_from_config(config: &StreamConfig, sample_f: &SampleFormat) -> WavSpec {
    WavSpec {
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
