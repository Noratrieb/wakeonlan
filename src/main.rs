// wake on lan code adapted from https://github.com/TeemuRemes/wake-on-lan-rust

use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use eyre::{bail, Context, ContextCompat};
use std::net::{Ipv4Addr, ToSocketAddrs, UdpSocket};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("info")))
        .init();

    // build our application with a route
    let app = Router::new()
        .route("/", get(async || Html(include_str!("../index.html"))))
        .route("/wake", post(wake));

    // run our app with hyper, listening globally on port 8090
    let addr = "0.0.0.0:8090";
    tracing::info!(?addr, "Starting server");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn wake() -> Response {
    tracing::info!("Waking");
    match tokio::task::spawn_blocking(|| wake_inner()).await {
        Ok(Ok(())) => (StatusCode::ACCEPTED, "sent packet").into_response(),
        Ok(Err(e)) => {
            tracing::error!(?e, "failed to wake");
            (StatusCode::INTERNAL_SERVER_ERROR, "error").into_response()
        }
        Err(e) => {
            tracing::error!(?e, "join error");
            (StatusCode::INTERNAL_SERVER_ERROR, "failed to spawn").into_response()
        }
    }
}

fn wake_inner() -> eyre::Result<()> {
    let hosts = load_possible_hosts()?;
    let host = hosts
        .iter()
        .find(|(host, _)| host.contains("PC-Nora"))
        .wrap_err_with(|| {
            format!(
                "failed to find host, found: {}",
                hosts
                    .iter()
                    .map(|(host, _)| host.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })?;
    let magic_packet = MagicPacket::new(&host.1);
    magic_packet.send().wrap_err("failed to send packet")?;

    tracing::info!(hostname = %host.0, mac = ?host.1, "Woke up");

    Ok(())
}

fn load_possible_hosts() -> eyre::Result<Vec<(String, [u8; 6])>> {
    // TODO: It would be very cool to instead read /proc/net/arp and then call getnameinfo but that's annoying...
    let arp = std::process::Command::new("arp")
        .output()
        .wrap_err("spwaning `arp`")?;
    if !arp.status.success() {
        bail!("arp failed: {}", String::from_utf8_lossy(&arp.stderr));
    }
    Ok(String::from_utf8(arp.stdout)
        .wrap_err("arp returned non-utf-8 output")?
        .lines()
        .skip(1)
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .map(|line_parts| {
            let mac = line_parts[2]
                .split(":")
                .map(|part| u8::from_str_radix(part, 16).expect("invalid mac address"))
                .collect::<Vec<_>>()
                .as_slice()
                .try_into()
                .expect("invalid mac address");
            (line_parts[0].to_owned(), mac)
        })
        .collect())
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
