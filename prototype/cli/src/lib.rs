pub mod cmd;
pub mod serial;
pub mod store;

#[cfg(feature = "pyo3")]
mod python_wrappers;
#[cfg(feature = "pyo3")]
pub use python_wrappers::*;

use folley_format::DeviceToServer;

use serial::TxPort;
use std::{io, sync::mpsc::Sender, thread, time::Duration};

pub fn connect(port_name: &str, tx: Sender<DeviceToServer>) -> io::Result<TxPort<32>> {
    let port = serialport::new(port_name, 460800)
        .flow_control(serialport::FlowControl::Hardware)
        .timeout(Duration::from_millis(500))
        .open()?;

    let tx_port = TxPort::new(port.try_clone().unwrap());

    let _rx_thread = thread::spawn(|| {
        serial::RxPort::new(port).run_read_task::<_, 20000>(move |msg| tx.send(msg).unwrap())
    });

    Ok(tx_port)
}

pub mod consts {
    use folley_calc::max_lags_size;

    pub const T_S_US: u32 = 22;
    pub const D_MICS_MM: u32 = 125;

    pub const SAMPLE_BUF_SIZE: usize = 1024;
    
    pub const LAG_TABLE_SIZE: usize = max_lags_size(T_S_US, D_MICS_MM);
    pub const XCORR_SIZE: usize = LAG_TABLE_SIZE;
}
