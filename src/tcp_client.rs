use log::info;
use mio::event::Event;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token};
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
        let mut i = 0;
        loop {
            // Poll Mio for events, blocking until we get an event.
            poll.poll(&mut events, None)?;

            // Process each event.
            for event in events.iter() {
                // We can use the token we previously provided to `register` to
                // determine for which socket the event is.
                match event.token() {
                    Self::CLIENT => {
                        self.handle_connection_event(&mut client, event);
                    }
                    // We don't expect any events with tokens other than those we provided.
                    _ => unreachable!(),
                }
            }
            i = i + 1;
            if i > 10 {
                break;
            }
        }
        return Ok(());
    }

    fn handle_connection_event(
        &self,
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        if event.is_writable() {
            match connection.write(&self.data) {
                Ok(n) if n < self.data.len() => {
                    let tmp = &self.data;
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
                    return self.handle_connection_event(connection, event);
                }
                // 他のエラーは致命的なエラーとして処理
                Err(err) => return Err(err),
            }
            // We can (likely) write to the socket without blocking.
        }

        if event.is_readable() {
            let mut connection_closed = false;
            let mut received_data = vec![0; 4096];
            let mut bytes_read = 0;
            // 該当の接続から受信できる可能性がある
            loop {
                match connection.read(&mut received_data[bytes_read..]) {
                    Ok(0) => {
                        // 0 bytes の受信の場合は、対抗が接続をクローズしたか、書き込みが完了している
                        connection_closed = true;
                        break;
                    }
                    Ok(n) => {
                        bytes_read += n;
                        if bytes_read == received_data.len() {
                            received_data.resize(received_data.len() + 1024, 0);
                        }
                    }
                    // Would block errors はOSがこのI/Oのオペレーションを実行する準備ができていないことを表す
                    Err(ref err) if self.would_block(err) => break,
                    Err(ref err) if self.interrupted(err) => continue,
                    // 他のエラーは致命的なエラーとして処理
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

        // Since the server just shuts down the connection, let's
        // just exit from our event loop.
        return Ok(false);
    }

    fn would_block(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}