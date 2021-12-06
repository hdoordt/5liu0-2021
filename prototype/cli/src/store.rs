use std::{
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};

use folley_format::device_to_server::SampleBuffer;

pub struct SampleStore<const N: usize> {
    writer: BufWriter<File>,
}

impl<const N: usize> SampleStore<N> {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::create(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn store(&mut self, samples: &SampleBuffer) -> Result<(), io::Error> {
        (0..SampleBuffer::size()).into_iter().try_for_each(|i| {
            write!(
                &mut self.writer,
                "{},{},{},{}\n",
                samples[i][0], samples[i][1], samples[i][2], samples[i][3],
            )
        })
    }
}
