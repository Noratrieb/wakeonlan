// adapted from https://github.com/TeemuRemes/wake-on-lan-rust

use std::net::{Ipv4Addr, ToSocketAddrs, UdpSocket};

fn main() {
    let mac_addr = std::env::args()
        .nth(1)
        .expect("first arg must be mac address");
    let parts = mac_addr
        .split(":")
        .map(|part| u8::from_str_radix(part, 16).expect("invalid mac address"))
        .collect::<Vec<_>>()
        .as_slice()
        .try_into()
        .expect("invalid mac address");

    let magic_packet = MagicPacket::new(&parts);
    magic_packet.send().expect("failed to send packet");

    println!("Done!");
}

/// A Wake-on-LAN magic packet.
pub struct MagicPacket {
    magic_bytes: [u8; 102],
}

impl MagicPacket {
    /// Creates a new `MagicPacket` intended for `mac_address` (but doesn't send it yet).
    pub fn new(mac_address: &[u8; 6]) -> MagicPacket {
        let mut magic_bytes: [u8; 102] = [0; 102];

        // We use `unsafe` code to skip unnecessary array initialization and bounds checking.
        unsafe {
            // Copy the header to the beginning.
            let mut src: *const u8 = &MAGIC_BYTES_HEADER[0];
            let mut dst: *mut u8 = &mut magic_bytes[0];
            dst.copy_from_nonoverlapping(src, 6);

            // Copy the MAC address once from the argument.
            src = &mac_address[0];
            dst = dst.offset(6);
            dst.copy_from_nonoverlapping(src, 6);

            // Repeat the MAC.
            let src: *const u8 = dst; // src points to magic_bytes[6]
            dst = dst.offset(6);
            dst.copy_from_nonoverlapping(src, 6);

            dst = dst.offset(6);
            dst.copy_from_nonoverlapping(src, 12);

            dst = dst.offset(12);
            dst.copy_from_nonoverlapping(src, 24);

            dst = dst.offset(24);
            dst.copy_from_nonoverlapping(src, 48);
        }

        MagicPacket { magic_bytes }
    }

    /// Sends the magic packet via UDP to the broadcast address `255.255.255.255:9`.
    /// Lets the operating system choose the source port and network interface.
    pub fn send(&self) -> std::io::Result<()> {
        self.send_to(
            (Ipv4Addr::new(255, 255, 255, 255), 9),
            (Ipv4Addr::new(0, 0, 0, 0), 0),
        )
    }

    /// Sends the magic packet via UDP to/from an IP address and port number of your choosing.
    pub fn send_to<A: ToSocketAddrs>(&self, to_addr: A, from_addr: A) -> std::io::Result<()> {
        let socket = UdpSocket::bind(from_addr)?;
        socket.set_broadcast(true)?;
        socket.send_to(&self.magic_bytes, to_addr)?;

        Ok(())
    }

    /// Returns the magic packet's payload (6 repetitions of `0xFF` and 16 repetitions of the
    /// target device's MAC address). Send these bytes yourself over the network if you want to do
    /// something more advanced (like reuse a single UDP socket when sending a large number of
    /// magic packets).
    pub fn magic_bytes(&self) -> &[u8; 102] {
        &self.magic_bytes
    }
}

const MAGIC_BYTES_HEADER: [u8; 6] = [0xFF; 6];
