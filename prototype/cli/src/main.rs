#[cfg(not(feature = "cli"))]
compile_error!("Please enable 'cli' feature to build CLI application");

use clap::{App, Arg};
use folley::serial::TxPort;
use folley::store::SampleStore;

use folley::consts::*;
use folley_format::DeviceToServer;
use serialport::{SerialPortType, UsbPortInfo};
use std::io::{self, BufRead};
use std::sync::mpsc;
use std::thread;

fn handle_message(msg: DeviceToServer) {
    use DeviceToServer::*;
    match msg {
        Samples(samples) => {
            let lag_table = folley_calc::gen_lag_table::<T_S_US, D_MICS_MM, LAG_TABLE_SIZE>();
            let mut buf = [0u32; XCORR_SIZE];
            let channels = folley_calc::Channels::from_samples(samples);
            let angle = folley_calc::calc_angle::<
                T_S_US,
                D_MICS_MM,
                XCORR_SIZE,
                SAMPLE_BUF_SIZE,
                LAG_TABLE_SIZE,
            >(&channels.ch1, &channels.ch2, &mut buf, &lag_table);

            dbg!(angle);
        }
        m => {
            println!("Unhandled message: {:?}", m);
        }
    }

    // println!("Got message: {:?}", msg);
    // TODO, do cool stuff with the message that just came in.
}

fn run<const N: usize>(mut tx_port: TxPort<N>) {
    use folley::cmd::Action::*;
    let stdin = io::stdin();
    let mut cmd = folley::cmd::Cmd::new();
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
    let matches = App::new("Folley commander")
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
    let mut store = matches
        .value_of("OUT_FILE")
        .map(|p| SampleStore::new(p).unwrap());

    let (tx, rx) = mpsc::channel::<DeviceToServer>();

    let rx_thread = thread::spawn(move || {
        for msg in rx.into_iter() {
            match msg {
                DeviceToServer::Samples(samples) => {
                    store
                        .as_mut()
                        .map(|s: &mut SampleStore<64>| s.store(&samples).unwrap());
                }
                _ => {}
            };
            handle_message(msg);
        }
    });

    if let Some(port_name) = matches.value_of("PORT") {
        if let Ok(tx_port) = folley::connect(port_name, tx) {
            run(tx_port);
            rx_thread.join().ok();
            return;
        }
    }
    eprintln!("Error connecting to port. Please specify port as the first argument. For help, run with --help");
    eprintln!();
    print_available_ports();
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
