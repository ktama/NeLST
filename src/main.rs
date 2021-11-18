mod initialize;
use initialize::file_config::CONFIG;
use log::{debug, error, info, warn};
use log4rs;
use std::io::{self};

mod tcp_server;

fn main() -> io::Result<()> {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    debug!("initilized logger");

    let is_server = CONFIG["load_test"]["is_server"].as_bool().unwrap();
    let protocol = CONFIG["load_test"]["protocol"].as_str().unwrap();
    let mode = (if is_server { "server" } else { "client" }, protocol);

    info!("Load Test Mode: {} & {}", mode.0, mode.1);
    match mode {
        ("client", "tcp") => {
            info!("Tcp Client");
            let target = CONFIG["load_test"]["target"].as_str().unwrap();
        }
        ("client", "udp") => {
            info!("Udp Client");
        }
        ("server", "tcp") => {
            info!("Tcp Server");
        }
        ("server", "udp") => {
            info!("Udp Server");
        }
        _ => error!("Errors in the configuration file"),
    }

    let bind_config_str = CONFIG["load_test"]["target"].as_str().unwrap();
    let bind_config = bind_config_str.parse().unwrap();
    let size_config_integer = CONFIG["load_test"]["packet_size"].as_integer().unwrap();
    let size_config = size_config_integer as usize;
    let tcp = tcp_server::TcpServer::new(bind_config, size_config);
    return tcp.test_traffic_load();
}
