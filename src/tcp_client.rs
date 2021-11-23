use log::info;
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Registry, Token};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::str::from_utf8;

pub struct TcpClient {
    target_addr: std::net::SocketAddr,
    data: Vec<u8>,
}

impl TcpClient {
    const CLIENT: Token = Token(1);

    pub fn new(target_addr_config: std::net::SocketAddr, packet_size_config: usize) -> TcpClient {
        info!(
            "config target_addr: {}, packet_size: {}",
            target_addr_config, packet_size_config
        );
        TcpClient {
            target_addr: target_addr_config,
            data: vec![0x31; packet_size_config],
        }
    }

    pub fn test_traffic_load(&self) -> io::Result<()> {
        let tmp = &self.data;
        if let Ok(str_buf) = from_utf8(tmp) {
            info!("Send data: {}", str_buf.trim_end());
        } else {
            info!("Send (none UTF-8) data: {:?}", tmp);
        }
        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(128);

        let mut client = TcpStream::connect(self.target_addr)?;
        poll.registry().register(
            &mut client,
            Self::CLIENT,
            Interest::READABLE | Interest::WRITABLE,
        )?;

        // Start an event loop.
        loop {
            // Poll Mio for events, blocking until we get an event.
            poll.poll(&mut events, None)?;

            // Process each event.
            for event in events.iter() {
                // We can use the token we previously provided to `register` to
                // determine for which socket the event is.
                match event.token() {
                    Self::CLIENT => {
                        if event.is_writable() {
                            match client.write(&self.data) {
                                Ok(n) if n < self.data.len() => {
                                    if let Ok(str_buf) = from_utf8(tmp) {
                                        info!("Sent data: {}", str_buf.trim_end());
                                    } else {
                                        info!("Sent (none UTF-8) data: {:?}", tmp);
                                    }
                                    return Err(io::ErrorKind::WriteZero.into());
                                }
                                Ok(_) => {}
                                Err(ref err) if self.would_block(err) => {}
                                Err(ref err) if self.interrupted(err) => {
                                    // TODO:割り込みが入った場合やり直す
                                }
                                // 他のエラーは致命的なエラーとして処理
                                Err(err) => return Err(err),
                            }
                            // We can (likely) write to the socket without blocking.
                        }

                        if event.is_readable() {
                            // We can (likely) read from the socket without blocking.
                        }

                        // Since the server just shuts down the connection, let's
                        // just exit from our event loop.
                        return Ok(());
                    }
                    // We don't expect any events with tokens other than those we provided.
                    _ => unreachable!(),
                }
            }
        }
    }

    fn would_block(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}
