use log::{debug, error, info, warn};
use log4rs;
use std::io::{self};

mod tcp_server;
// TODO: WARNING unused
use tcp_server::TcpServer;

fn main() -> io::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    error!("error log");
    warn!("warn log");
    info!("info log");
    debug!("debug log");

    let tcp = tcp_server::TcpServer::new();
    return tcp.test_traffic_load();
}
