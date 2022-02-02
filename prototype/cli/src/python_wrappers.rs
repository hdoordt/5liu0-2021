use std::sync::{mpsc, Mutex};

use once_cell::sync::Lazy;
use pyo3::prelude::*;
static SAMPLES: Lazy<Mutex<[Vec<i16>; 4]>> =
    Lazy::new(|| Mutex::new([vec![], vec![], vec![], vec![]]));

#[pyfunction]
fn init(port_name: String, compress_factor: usize) -> PyResult<()> {
    use std::thread;

    use folley_format::DeviceToServer;
    assert!(
        compress_factor > 0,
        "Compress factor must be greater than 0"
    );
    let (tx, rx) = mpsc::channel::<DeviceToServer>();

    let _tx_port = crate::connect(&port_name, tx)?;

    thread::spawn(move || {
        for msg in rx.into_iter() {
            if let DeviceToServer::Samples(samples) = msg {
                let mut compressed =
                    Vec::with_capacity((samples.len() + compress_factor) / compress_factor);
                for i in 0..samples.len() / compress_factor {
                    let y = &samples[i..i + compress_factor]
                        .iter()
                        .fold([0, 0, 0, 0], |r, s| {
                            [
                                r[0] + s[0] as i32,
                                r[1] + s[1] as i32,
                                r[2] + s[2] as i32,
                                r[3] + s[3] as i32,
                            ]
                        });
                    let cf = compress_factor as i32;
                    compressed.push([
                        (y[0] / cf) as i16,
                        (y[1] / cf) as i16,
                        (y[2] / cf) as i16,
                        (y[3] / cf) as i16,
                    ]);
                }
                let mut buf = SAMPLES.lock().unwrap();
                for i in 0..4 {
                    buf[i].extend(compressed.iter().map(|c| c[i]));
                }
            }
        }
    });

    PyResult::Ok(())
}

#[pyfunction]
fn get_samples() -> PyResult<[Vec<i16>; 4]> {
    let samples = std::mem::replace(
        &mut *SAMPLES.lock().unwrap(),
        [vec![], vec![], vec![], vec![]],
    );
    Ok(samples)
}

#[pymodule]
fn folley(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(init, m)?)?;
    m.add_function(wrap_pyfunction!(get_samples, m)?)?;
    Ok(())
}
