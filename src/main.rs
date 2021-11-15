mod initialize;
use initialize::file_config::CONFIG;
use log::{debug, error, info, warn};
use log4rs;
use std::io::{self};

mod tcp_server;

fn main() -> io::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();

    error!("error log");
    warn!("warn log");
    info!("info log");
    debug!("debug log");
    let bind_config_str = CONFIG["load_test"]["target"].as_str().unwrap();
    let bind_config = bind_config_str.parse().unwrap();
    let size_config_integer = CONFIG["load_test"]["packet_size"].as_integer().unwrap();
    let size_config = size_config_integer as usize;
    let tcp = tcp_server::TcpServer::new(bind_config, size_config);
    return tcp.test_traffic_load();
}
