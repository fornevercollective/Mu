use crate::packet::MuPacket;

/// MuVis Vision Bridge.
///
/// Inspects each burst of [`MuPacket`] frames for the trigger bit, using a
/// branchless O(1) XOR to toggle the 120 Hz display state.  Cumulative
/// inspection across bursts prevents UI "strobing" when partial bursts arrive
/// out of phase.
pub struct MuVis {
    trigger_count: u32,
    hz_state: bool,
}

impl MuVis {
    pub fn new() -> Self {
        Self {
            trigger_count: 0,
            hz_state: false,
        }
    }

    /// Process a slice of packets, toggling the 120 Hz state for each
    /// packet whose trigger bit is set.
    pub fn render(&mut self, packets: &[MuPacket]) {
        for packet in packets {
            // Branchless O(1) XOR toggle: no branch predictor pressure.
            let triggered = (packet.header & MuPacket::TRIGGER_BIT) != 0;
            self.trigger_count += triggered as u32;
            self.hz_state ^= triggered;
        }
    }

    #[allow(dead_code)]
    pub fn trigger_count(&self) -> u32 {
        self.trigger_count
    }

    #[allow(dead_code)]
    pub fn hz_state(&self) -> bool {
        self.hz_state
    }
}

impl Default for MuVis {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_toggles_hz_state() {
        let mut vis = MuVis::new();
        // First packet has trigger bit set — hz_state should flip to true.
        let p0 = MuPacket::new(0, b"first");
        vis.render(&[p0]);
        assert!(vis.hz_state());
        assert_eq!(vis.trigger_count(), 1);

        // Second packet has no trigger bit — hz_state unchanged.
        let p1 = MuPacket::new(1, b"second");
        vis.render(&[p1]);
        assert!(vis.hz_state());
        assert_eq!(vis.trigger_count(), 1);
    }

    #[test]
    fn render_248_packet_burst_counts_one_trigger() {
        let mut vis = MuVis::new();
        let pkts: Vec<MuPacket> = (0u16..248).map(|i| MuPacket::new(i, b"data")).collect();
        vis.render(&pkts);
        assert_eq!(vis.trigger_count(), 1);
    }
}
