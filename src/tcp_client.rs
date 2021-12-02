use log::{debug, info};
use mio::event::Event;
use mio::net::TcpStream;
use mio::{Events, Interest, Poll, Token, Waker};
use std::io::{self, Read, Write};
use std::str::from_utf8;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub struct TcpClient {
    target_addr: std::net::SocketAddr,
    data: Vec<u8>,
}

impl TcpClient {
    const CLIENT: Token = Token(2);
    const WAKER: Token = Token(1);

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

    pub fn test_traffic_load(&self, handle: &str) -> io::Result<()> {
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

        let waker = Arc::new(Waker::new(poll.registry(), Self::WAKER)?);
        let waker_clone = waker.clone();
        let counter = Arc::new(RwLock::new(0));
        {
            let counter = Arc::clone(&counter);

            thread::spawn(move || {
                while *counter.read().unwrap() < 100 {
                    thread::sleep(Duration::from_nanos(1000));
                    info!("wake {}", *counter.read().unwrap());
                    {
                        let mut w = counter.write().unwrap();
                        *w += 1;
                    }
                    waker_clone.wake().expect("unable to wake");
                }
            });
        }

        loop {
            poll.poll(&mut events, None)?;
            for event in events.iter() {
                debug!(
                    "Event Token: {:?}, Writable: {}, Readable: {}",
                    event.token(),
                    event.is_writable(),
                    event.is_readable()
                );
                match event.token() {
                    Self::CLIENT | Self::WAKER => match handle {
                        "send only" => {
                            match self.handle_send_only_connection_event(&mut client, event) {
                                // 接続維持
                                Ok(false) => {}
                                // 接続終了
                                Ok(true) => return Ok(()),
                                Err(err) => return Err(err),
                            }
                        }
                        "to echo server" => {
                            match self.handle_echo_server_connection_event(&mut client, event) {
                                // 接続維持
                                Ok(false) => {}
                                // 接続終了
                                Ok(true) => return Ok(()),
                                Err(err) => return Err(err),
                            }
                        }
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                }
            }
            debug!("count {}", *counter.read().unwrap());
            if *counter.read().unwrap() >= 100 {
                info!("end");
                break;
            }
        }
        return Ok(());
    }

    fn handle_send_only_connection_event(
        &self,
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        match connection.write(&self.data) {
            Ok(n) if n < self.data.len() => {
                return Err(io::ErrorKind::WriteZero.into());
            }
            Ok(_) => {
                let tmp = &self.data;
                if let Ok(str_buf) = from_utf8(tmp) {
                    info!("Sent data: {}", str_buf.trim_end());
                } else {
                    info!("Sent (none UTF-8) data: {:?}", tmp);
                }
                return Ok(true);
            }
            Err(ref err) if self.would_block(err) => {}
            Err(ref err) if self.interrupted(err) => {
                return self.handle_send_only_connection_event(connection, event);
            }
            // 他のエラーは致命的なエラーとして処理
            Err(err) => return Err(err),
        }
        return Ok(false);
    }

    fn handle_echo_server_connection_event(
        &self,
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        // Wake のタイミングのみ書き込む、clientの受信イベントはis_writable()=trueのため
        if !event.is_writable() {
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
                    return self.handle_echo_server_connection_event(connection, event);
                }
                // 他のエラーは致命的なエラーとして処理
                Err(err) => return Err(err),
            }
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
        return Ok(false);
    }

    fn would_block(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}
