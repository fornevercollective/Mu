use std::io::{self, Read};
use std::os::unix::net::UnixStream;

use crate::packet::MuPacket;

/// Mu-Pipe Receiver (`stream_apple`).
///
/// Reads a burst of [`MuPacket`] frames from a Unix-domain socket using a
/// 64 KB stack-allocated L2 buffer.  The inner decode loop is manually
/// unrolled four packets at a time (128 bytes per iteration) to saturate
/// NEON/AMX load pipelines.  A fragment-recovery shim handles the tail when
/// the burst length is not a multiple of four.
pub struct MuPipeReceiver {
    stream: UnixStream,
}

impl MuPipeReceiver {
    pub fn new(stream: UnixStream) -> Self {
        Self { stream }
    }

    /// Read exactly `expected` packets from the socket.
    ///
    /// The entire burst is pulled into a 64 KB stack buffer with a single
    /// `read` call sequence, then decoded in place.
    pub fn read_burst(&mut self, expected: usize) -> io::Result<Vec<MuPacket>> {
        // 64 KB stack-allocated L2 buffer — large enough for any standard burst.
        let mut buf = [0u8; 65536];
        let total_bytes = expected * MuPacket::SIZE;

        // Pull the whole burst (may arrive in multiple kernel segments).
        let mut received = 0;
        while received < total_bytes {
            match self.stream.read(&mut buf[received..total_bytes]) {
                Ok(0) => break, // EOF / sender closed
                Ok(n) => received += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }

        let full_packets = received / MuPacket::SIZE;
        let mut packets = Vec::with_capacity(full_packets);
        let mut offset = 0;

        // Manual 4-packet unroll: 128 bytes per iteration.
        let unroll_end = (full_packets / 4) * 4 * MuPacket::SIZE;
        while offset < unroll_end {
            for i in 0..4 {
                let start = offset + i * MuPacket::SIZE;
                let bytes: &[u8; 32] = buf[start..start + 32].try_into().unwrap();
                packets.push(*MuPacket::from_bytes(bytes));
            }
            offset += 4 * MuPacket::SIZE;
        }

        // Fragment-recovery shim: handle the remaining 0–3 packets.
        while offset + MuPacket::SIZE <= received {
            let bytes: &[u8; 32] = buf[offset..offset + 32].try_into().unwrap();
            packets.push(*MuPacket::from_bytes(bytes));
            offset += MuPacket::SIZE;
        }

        Ok(packets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::muon_dispatch_v2;
    use std::io::Write;

    #[test]
    fn read_burst_recovers_all_packets() {
        let (mut tx, rx) = std::os::unix::net::UnixStream::pair().unwrap();
        let data: Vec<u8> = (0u8..=255).cycle().take(248 * 24).collect();
        let sent = muon_dispatch_v2(&data);

        let handle = std::thread::spawn(move || {
            for pkt in &sent {
                tx.write_all(pkt.as_bytes()).unwrap();
            }
        });

        let mut receiver = MuPipeReceiver::new(rx);
        let received = receiver.read_burst(248).unwrap();
        handle.join().unwrap();

        assert_eq!(received.len(), 248);
        assert_eq!(received[0].header & MuPacket::TRIGGER_BIT, MuPacket::TRIGGER_BIT);
        assert_eq!(received[1].header & MuPacket::TRIGGER_BIT, 0);
    }
}
