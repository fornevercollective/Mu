use std::mem;

/// A 32-byte MuPacket structure for zero-copy UDS streaming.
///
/// Layout (repr C, no padding):
///   header  : u8        @ offset  0 — trigger bit (0x80) set on the first packet of a burst
///   flags   : u8        @ offset  1 — reserved frame flags
///   seq     : u16       @ offset  2 — sequence number within a burst
///   len     : u32       @ offset  4 — byte count of valid payload data
///   payload : [u8; 24]  @ offset  8 — raw payload bytes
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct MuPacket {
    pub header: u8,
    pub flags: u8,
    pub seq: u16,
    pub len: u32,
    pub payload: [u8; 24],
}

// Compile-time guarantee that no padding has been inserted.
const _MUPACKET_SIZE_CHECK: () = assert!(mem::size_of::<MuPacket>() == 32);

impl MuPacket {
    pub const SIZE: usize = 32;
    pub const TRIGGER_BIT: u8 = 0x80;

    /// Build a new packet, copying up to 24 bytes from `payload`.
    /// The trigger bit is set only on the first packet (seq == 0).
    pub fn new(seq: u16, payload: &[u8]) -> Self {
        let len = payload.len().min(24) as u32;
        let mut p = MuPacket {
            header: if seq == 0 { Self::TRIGGER_BIT } else { 0 },
            flags: 0,
            seq,
            len,
            payload: [0u8; 24],
        };
        p.payload[..len as usize].copy_from_slice(&payload[..len as usize]);
        p
    }

    /// Zero-copy view of the packet as a raw byte slice.
    ///
    /// Safety: `MuPacket` is `#[repr(C)]` with no padding, so every bit pattern
    /// is a valid sequence of bytes.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self as *const _ as *const u8, Self::SIZE) }
    }

    /// Reinterpret a 32-byte slice as a `MuPacket` reference.
    ///
    /// Safety: `MuPacket` is `#[repr(C)]` with no padding and all fields accept
    /// any bit pattern, so every 32-byte slice is a valid `MuPacket`.
    pub fn from_bytes(bytes: &[u8; 32]) -> &Self {
        unsafe { &*(bytes.as_ptr() as *const Self) }
    }
}

/// Maps raw AI-output bytes into a `Vec<MuPacket>` (muon_dispatch_v2).
///
/// Each packet carries up to 24 bytes of payload.  The first packet in the
/// stream has its trigger bit set so that the Vision Bridge can detect burst
/// boundaries without inspecting every payload.
pub fn muon_dispatch_v2(data: &[u8]) -> Vec<MuPacket> {
    data.chunks(24)
        .enumerate()
        .map(|(i, chunk)| MuPacket::new(i as u16, chunk))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mupacket_is_32_bytes() {
        assert_eq!(mem::size_of::<MuPacket>(), 32);
    }

    #[test]
    fn trigger_bit_only_on_first_packet() {
        let p0 = MuPacket::new(0, b"hello");
        let p1 = MuPacket::new(1, b"world");
        assert_eq!(p0.header & MuPacket::TRIGGER_BIT, MuPacket::TRIGGER_BIT);
        assert_eq!(p1.header & MuPacket::TRIGGER_BIT, 0);
    }

    #[test]
    fn muon_dispatch_v2_produces_correct_packet_count() {
        let data: Vec<u8> = vec![0u8; 248 * 24];
        let pkts = muon_dispatch_v2(&data);
        assert_eq!(pkts.len(), 248);
    }

    #[test]
    fn as_bytes_round_trips_via_from_bytes() {
        let pkt = MuPacket::new(7, b"round_trip_test_data");
        let bytes: &[u8; 32] = pkt.as_bytes().try_into().unwrap();
        let restored = MuPacket::from_bytes(bytes);
        assert_eq!(restored.seq, 7);
        assert_eq!(restored.len, 20);
        assert_eq!(&restored.payload[..20], b"round_trip_test_data");
    }
}
