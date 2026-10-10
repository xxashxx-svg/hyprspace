use flate2::{Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status};

pub struct Squeeze(Compress);

impl Default for Squeeze {
    fn default() -> Self {
        Self(Compress::new(Compression::fast(), false))
    }
}

impl Squeeze {
    pub fn run(&mut self, mut data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() / 4 + 64);
        loop {
            out.reserve(4096);
            let before = self.0.total_in();
            if self
                .0
                .compress_vec(data, &mut out, FlushCompress::Sync)
                .is_err()
            {
                break;
            }
            data = &data[(self.0.total_in() - before) as usize..];
            if data.is_empty() && out.len() < out.capacity() {
                break;
            }
        }
        out
    }
}

pub struct Unsqueeze(Decompress);

impl Default for Unsqueeze {
    fn default() -> Self {
        Self(Decompress::new(false))
    }
}

impl Unsqueeze {
    pub fn run(&mut self, mut data: &[u8]) -> Option<Vec<u8>> {
        let mut out = Vec::with_capacity(data.len() * 4 + 64);
        loop {
            out.reserve(16384);
            let before = self.0.total_in();
            let status = self
                .0
                .decompress_vec(data, &mut out, FlushDecompress::Sync)
                .ok()?;
            data = &data[(self.0.total_in() - before) as usize..];
            if status == Status::StreamEnd || (data.is_empty() && out.len() < out.capacity()) {
                return Some(out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_squeeze_and_come_back_in_order() {
        let (mut s, mut u) = (Squeeze::default(), Unsqueeze::default());
        let screen = "\x1b[H\x1b[2J".to_string() + &"a line of output that repeats\r\n".repeat(400);
        let first = s.run(screen.as_bytes());
        assert!(first.len() * 10 < screen.len(), "{}", first.len());
        assert_eq!(u.run(&first).unwrap(), screen.as_bytes());
        let second = s.run(screen.as_bytes());
        assert!(second.len() < first.len());
        assert_eq!(u.run(&second).unwrap(), screen.as_bytes());
        let big: Vec<u8> = (0..300_000u32).map(|i| (i * 7919 % 251) as u8).collect();
        assert_eq!(u.run(&s.run(&big)).unwrap(), big);
        assert_eq!(u.run(&s.run(b"")).unwrap(), b"");
    }
}
