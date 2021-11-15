use log::info;
use mio::event::Event;
use mio::net::{TcpListener, TcpStream};
use mio::{Events, Interest, Poll, Registry, Token};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::str::from_utf8;

pub struct TcpServer {
    bind_addr: std::net::SocketAddr,
    data: Vec<u8>,
}

impl TcpServer {
    const SERVER: Token = Token(0);

    pub fn new(bind_addr_config: std::net::SocketAddr, packet_size_config: usize) -> TcpServer {
        info!(
            "[config] bind_addr: {}, packet_size: {}",
            bind_addr_config, packet_size_config
        );
        TcpServer {
            bind_addr: bind_addr_config,
            data: vec![0x1; packet_size_config],
        }
    }

    pub fn test_traffic_load(&self) -> io::Result<()> {
        let tmp = &self.data;
        if let Ok(str_buf) = from_utf8(tmp) {
            info!("Received data: {}", str_buf.trim_end());
        } else {
            info!("Received (none UTF-8) data: {:?}", tmp);
        }
        info!("data: {:?}", tmp);
        // Create a poll instance.
        let mut poll = Poll::new()?;
        // Create storage for events.
        let mut events = Events::with_capacity(128);

        // Setup the server socket.
        let mut server = TcpListener::bind(self.bind_addr)?;
        // Start listening for incoming connections.
        poll.registry()
            .register(&mut server, Self::SERVER, Interest::READABLE)?;

        // Map of `Token` -> `TcpStream`.
        let mut connections = HashMap::new();
        // Unique token for each incoming connection.
        let mut unique_token = Token(Self::SERVER.0 + 1);

        // Start an event loop.
        loop {
            // Poll Mio for events, blocking until we get an event.
            poll.poll(&mut events, None)?;

            // Process each event.
            for event in events.iter() {
                // We can use the token we previously provided to `register` to
                // determine for which socket the event is.
                match event.token() {
                    Self::SERVER => loop {
                        // If this is an event for the server, it means a connection
                        // is ready to be accepted.
                        //
                        // Accept the connection and drop it immediately. This will
                        // close the socket and notify the client of the EOF.
                        let (mut connection, address) = match server.accept() {
                            Ok((connection, address)) => (connection, address),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // If we get a `WouldBlock` error we know our
                                // listener has no more incoming connections queued,
                                // so we can return to polling and wait for some
                                // more.
                                break;
                            }
                            Err(e) => {
                                // If it was any other kind of error, something went
                                // wrong and we terminate with an error.
                                return Err(e);
                            }
                        };

                        info!("Accepted connection from: {}", address);

                        let token = self.next(&mut unique_token);
                        poll.registry().register(
                            &mut connection,
                            token,
                            Interest::READABLE.add(Interest::WRITABLE),
                        )?;

                        connections.insert(token, connection);
                    },
                    token => {
                        // Maybe received an event for a TCP connection.
                        let done = if let Some(connection) = connections.get_mut(&token) {
                            self.handle_connection_event(poll.registry(), connection, event)?
                        } else {
                            // Sporadic events happen, we can safely ignore them.
                            false
                        };
                        if done {
                            if let Some(mut connection) = connections.remove(&token) {
                                poll.registry().deregister(&mut connection)?;
                            }
                        }
                    }
                }
            }
        }
    }

    fn next(&self, current: &mut Token) -> Token {
        let next = current.0;
        current.0 += 1;
        Token(next)
    }

    /// Returns `true` if the connection is done.
    fn handle_connection_event(
        &self,
        registry: &Registry,
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        if event.is_writable() {
            // We can (maybe) write to the connection.
            match connection.write(&self.data) {
                // We want to write the entire `DATA` buffer in a single go. If we
                // write less we'll return a short write error (same as
                // `io::Write::write_all` does).
                Ok(n) if n < self.data.len() => return Err(io::ErrorKind::WriteZero.into()),
                Ok(_) => {
                    // After we've written something we'll reregister the connection
                    // to only respond to readable events.
                    registry.reregister(connection, event.token(), Interest::READABLE)?
                }
                // Would block "errors" are the OS's way of saying that the
                // connection is not actually ready to perform this I/O operation.
                Err(ref err) if self.would_block(err) => {}
                // Got interrupted (how rude!), we'll try again.
                Err(ref err) if self.interrupted(err) => {
                    return self.handle_connection_event(registry, connection, event)
                }
                // Other errors we'll consider fatal.
                Err(err) => return Err(err),
            }
        }

        if event.is_readable() {
            let mut connection_closed = false;
            let mut received_data = vec![0; 4096];
            let mut bytes_read = 0;
            // We can (maybe) read from the connection.
            loop {
                match connection.read(&mut received_data[bytes_read..]) {
                    Ok(0) => {
                        // Reading 0 bytes means the other side has closed the
                        // connection or is done writing, then so are we.
                        connection_closed = true;
                        break;
                    }
                    Ok(n) => {
                        bytes_read += n;
                        if bytes_read == received_data.len() {
                            received_data.resize(received_data.len() + 1024, 0);
                        }
                    }
                    // Would block "errors" are the OS's way of saying that the
                    // connection is not actually ready to perform this I/O operation.
                    Err(ref err) if self.would_block(err) => break,
                    Err(ref err) if self.interrupted(err) => continue,
                    // Other errors we'll consider fatal.
                    Err(err) => return Err(err),
                }
            }

            if bytes_read != 0 {
                let received_data = &received_data[..bytes_read];
                if let Ok(str_buf) = from_utf8(received_data) {
                    info!("Received data: {}", str_buf.trim_end());
                } else {
                    info!("Received (none UTF-8) data: {:?}", received_data);
                }
            }

            if connection_closed {
                info!("Connection closed");
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn would_block(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}
