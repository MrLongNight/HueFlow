use anyhow::{Context, Result};
use openssl::ssl::{SslConnector, SslMethod, SslStream};
use std::io::{self, Read, Write};
use std::net::UdpSocket;
use std::time::Duration;

// Wrapper for UdpSocket to implement Read and Write
struct ConnectedUdpSocket(UdpSocket);

impl Read for ConnectedUdpSocket {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }
}

impl Write for ConnectedUdpSocket {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Debugging packet sizes
        println!("UDP Write: {} bytes", buf.len());
        self.0.send(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct HueStreamer {
    stream: SslStream<ConnectedUdpSocket>,
}

impl HueStreamer {
    pub fn connect(ip: &str, username: &str, psk: &str) -> Result<Self> {
        let addr = format!("{}:2100", ip);

        // Setup UDP Socket
        let socket = UdpSocket::bind("0.0.0.0:0").context("Failed to bind UDP socket")?;
        socket
            .connect(&addr)
            .context("Failed to connect UDP socket")?;

        // Set timeouts
        socket.set_read_timeout(Some(Duration::from_secs(2))).ok();
        socket.set_write_timeout(Some(Duration::from_secs(2))).ok();

        // Wrap socket
        let socket_wrapper = ConnectedUdpSocket(socket);

        // Setup OpenSSL Connector
        let mut builder = SslConnector::builder(SslMethod::dtls())
            .context("Failed to create SslConnector builder")?;

        // Explicitly enable DTLS 1.2 (disable 1.0)
        builder.set_options(openssl::ssl::SslOptions::NO_DTLSV1);

        // Cipher List
        builder
            .set_cipher_list("PSK-AES128-GCM-SHA256")
            .context("Failed to set cipher list")?;

        // PSK Callback
        let username = username.to_string();
        let psk_hex = psk.to_string();

        builder.set_psk_client_callback(move |_, _, identity, psk_buf| {
            // Identity
            let identity_bytes = username.as_bytes();
            if identity_bytes.len() > identity.len() {
                return Err(openssl::error::ErrorStack::get());
            }
            identity[..identity_bytes.len()].copy_from_slice(identity_bytes);

            if identity_bytes.len() < identity.len() {
                identity[identity_bytes.len()] = 0;
            }

            // PSK
            let key_bytes = match hex::decode(&psk_hex) {
                Ok(k) => k,
                Err(_) => return Err(openssl::error::ErrorStack::get()),
            };

            if key_bytes.len() > psk_buf.len() {
                return Err(openssl::error::ErrorStack::get());
            }
            psk_buf[..key_bytes.len()].copy_from_slice(&key_bytes);

            Ok(key_bytes.len())
        });

        let connector = builder.build();

        // Handshake
        let mut ssl = connector.configure()?.into_ssl(&addr)?;

        // Set MTU explicitly to avoid fragmentation issues
        ssl.set_mtu(1400).ok();

        // Use SslStream::new to create the stream, then call connect()
        let mut stream = SslStream::new(ssl, socket_wrapper)
            .map_err(|e| anyhow::anyhow!("Failed to create SslStream: {}", e))?;

        stream
            .connect()
            .map_err(|e| anyhow::anyhow!("DTLS Handshake failed: {}", e))?;

        Ok(HueStreamer { stream })
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.stream
            .write_all(buf)
            .context("Failed to write to DTLS stream")?;
        self.stream.flush().context("Failed to flush DTLS stream")?;
        Ok(())
    }
}
