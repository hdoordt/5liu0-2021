use clap::{App, Arg};
use folley_format::device_to_server::SampleBuffer;
use folley_format::DeviceToServer;
use serialport::{SerialPortType, UsbPortInfo};
use std::io::{self, BufRead};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;
use store::SampleStore;

use crate::serial::TxPort;

mod cmd;
mod serial;
mod store;

fn handle_message(msg: DeviceToServer, store_tx: Option<Sender<SampleBuffer>>) {
    // println!("Got message: {:?}", msg);

    if let Some(buf) = msg.samples {
        store_tx.map(|t| t.send(buf));
    }
    // TODO, do cool stuff with the message that just came in.
}

fn run<const N: usize>(mut tx_port: TxPort<N>) {
    use cmd::Action::*;
    let stdin = io::stdin();
    let mut cmd = cmd::Cmd::new();
    print!("--> ");
    for line in stdin.lock().lines().filter_map(|r| r.ok()) {
        match cmd.parse_line(&line) {
            SendMessage(msg) => {
                tx_port.write_message(&msg).unwrap();
            }
            a => eprintln!("Unknown action {:?}", a),
        }
        print!("--> ");
    }
}

fn main() {
    let matches = App::new("Device commander")
        .version("0.1")
        .arg(
            Arg::with_name("OUT_FILE")
                .short("o")
                .long("outfile")
                .required(false)
                .takes_value(true)
                .help("The path of the file to write to"),
        )
        .arg(
            Arg::with_name("PORT")
                .index(1)
                .takes_value(true)
                .help("The path to the serial port to listen to"),
        )
        .get_matches();
    let store = matches
        .value_of("OUT_FILE")
        .map(|p| SampleStore::new(p).unwrap());

    if let Some(port_name) = matches.value_of("PORT") {
        listen(port_name, store)
    } else {
        eprintln!("Please specify port as the first argument. For help, run with --help");
        eprintln!();
        print_available_ports();
    }
}

fn listen(port_name: &str, store: Option<SampleStore<64>>) {
    let port = serialport::new(port_name, 115200)
        .flow_control(serialport::FlowControl::Hardware)
        .timeout(Duration::from_millis(500))
        .open();

    let (store_tx, store_thread) = match store {
        Some(mut s) => {
            let (tx, rx) = mpsc::channel();
            let thread = thread::spawn(move || {
                while let Ok(samples) = rx.recv() {
                    s.store(samples).unwrap();
                }
            });
            (Some(tx), Some(thread))
        }
        _ => (None, None),
    };

    match port {
        Ok(port) => {
            let tx_port: TxPort<32> = TxPort::new(port.try_clone().unwrap());

            let rx_thread = thread::spawn(|| {
                serial::RxPort::new(port)
                    .run_read_task::<_, 4096>(move |msg| handle_message(msg, store_tx.clone()))
            });

            run(tx_port);

            rx_thread.join().unwrap();
        }
        Err(e) => {
            eprintln!("Error opening serial port {}: {}", port_name, e);
            eprintln!();
            print_available_ports();
        }
    }
    store_thread.map(|t| t.join().unwrap());
}

fn print_available_ports() {
    println!("Available ports (listing USB only):");
    for port in serialport::available_ports().unwrap() {
        match (port.port_name, port.port_type) {
            (
                port_name,
                SerialPortType::UsbPort(UsbPortInfo {
                    vid,
                    pid,

                    manufacturer,
                    ..
                }),
            ) => {
                let manufacturer = manufacturer.unwrap_or_default();
                eprintln!(
                    "\t - {} (Vendor ID: {:#x}; Product ID: {:#x}; Manufacturer: {})",
                    port_name, vid, pid, manufacturer,
                );
            }
            _ => {} // Ignore other types
        }
    }
}
