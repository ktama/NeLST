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
            "config bind_addr: {}, packet_size: {}",
            bind_addr_config, packet_size_config
        );
        TcpServer {
            bind_addr: bind_addr_config,
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
        // pollのインスタンスを作成
        let mut poll = Poll::new()?;
        // eventのストレージ領域
        let mut events = Events::with_capacity(128);

        // サーバーソケットを設定
        let mut server = TcpListener::bind(self.bind_addr)?;
        // 着信接続のリスニングを開始
        poll.registry()
            .register(&mut server, Self::SERVER, Interest::READABLE)?;

        // `Token` -> `TcpStream` のマップ
        let mut connections = HashMap::new();
        //  着信接続のユニークトークン
        let mut unique_token = Token(Self::SERVER.0 + 1);

        // Start an event loop.
        loop {
            // イベントが発生するまで待機 Poll Mio
            poll.poll(&mut events, None)?;

            // 各イベントの処理
            for event in events.iter() {
                // "register" に登録したトークンをを利用して、どのソケットのイベントか判断できる
                match event.token() {
                    Self::SERVER => loop {
                        // サーバーのイベントの場合、接続準備ができていることを意味する
                        // 接続を許可し、すぐにドロップする
                        // これにより、ソケットがクローズされ、クライアントへEOFを通知
                        let (mut connection, address) = match server.accept() {
                            Ok((connection, address)) => (connection, address),
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                                // `WouldBlock` エラーが発生した場合、リスナーは着信接続がキューにないことがわかるので、
                                // ポーリングに戻り次の接続を待つ。
                                break;
                            }
                            Err(e) => {
                                // 他の種類のエラーの場合は、何かの誤りがあるため終了
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
                        // TCP接続を受信した可能性がある
                        let done = if let Some(connection) = connections.get_mut(&token) {
                            self.handle_connection_event(poll.registry(), connection, event)?
                        } else {
                            // まばらなイベントが発生した場合は無視できる
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

    /// 接続が完了した場合、`true`を返す
    fn handle_connection_event(
        &self,
        registry: &Registry,
        connection: &mut TcpStream,
        event: &Event,
    ) -> io::Result<bool> {
        if event.is_writable() {
            // 該当の接続へ書き込みできる可能性がある
            match connection.write(&self.data) {
                // バッファへ`DATA`を一度に書き込む
                // `DATA`より書き込めた長さが短い場合、書き込みエラーを返す
                // `io::Write::write_all` と同様の動き
                Ok(n) if n < self.data.len() => {
                    let tmp = &self.data;
                    if let Ok(str_buf) = from_utf8(tmp) {
                        info!("Sent data: {}", str_buf.trim_end());
                    } else {
                        info!("Sent (none UTF-8) data: {:?}", tmp);
                    }
                    return Err(io::ErrorKind::WriteZero.into());
                }
                Ok(_) => {
                    // 書き込み後は、受信イベントのみに反応するように接続を再登録
                    registry.reregister(connection, event.token(), Interest::READABLE)?
                }
                // Would block errors はOSがこのI/Oのオペレーションを実行する準備ができていないことを表す
                Err(ref err) if self.would_block(err) => {}
                // 割り込みが入った場合やり直す
                Err(ref err) if self.interrupted(err) => {
                    return self.handle_connection_event(registry, connection, event)
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

        Ok(false)
    }

    fn would_block(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::WouldBlock
    }

    fn interrupted(&self, err: &io::Error) -> bool {
        err.kind() == io::ErrorKind::Interrupted
    }
}
