pub mod cmd;
pub mod serial;
pub mod store;

#[cfg(feature="pyo3")]
mod python_wrappers;
#[cfg(feature="pyo3")]
pub use python_wrappers::*;

use folley_format::DeviceToServer;

use serial::TxPort;
use std::{io, sync::mpsc::Sender, thread, time::Duration};

pub fn connect(port_name: &str, tx: Sender<DeviceToServer>) -> io::Result<TxPort<32>> {
    let port = serialport::new(port_name, 1000000)
        .flow_control(serialport::FlowControl::Hardware)
        .timeout(Duration::from_millis(500))
        .open()?;

    let tx_port= TxPort::new(port.try_clone().unwrap());

    let _rx_thread = thread::spawn(|| {
        serial::RxPort::new(port).run_read_task::<_, 16384>(move |msg| tx.send(msg).unwrap())
    });

    Ok(tx_port)
}
