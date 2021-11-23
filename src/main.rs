mod initialize;
use initialize::file_config::CONFIG;
use log::{debug, error, info};
use log4rs;
use std::net::SocketAddr;

mod tcp_client;
mod tcp_server;

fn main() {
    log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    debug!("initilized logger");

    let is_server = CONFIG["load_test"]["is_server"].as_bool().unwrap();
    let protocol = CONFIG["load_test"]["protocol"].as_str().unwrap();
    let mode = (if is_server { "server" } else { "client" }, protocol);

    execute_load_test(mode);
}

pub fn execute_load_test(mode: (&str, &str)) {
    info!("Load Test Mode: {} & {}", mode.0, mode.1);
    match mode {
        ("client", "tcp") => {
            info!("Tcp Client");
            let target = CONFIG["load_test"]["target"].as_str().unwrap();
            let target_addr = target.parse::<SocketAddr>().unwrap();
            let size_config_integer = CONFIG["load_test"]["packet_size"].as_integer().unwrap();
            let size_config = size_config_integer as usize;
            let udp = tcp_client::TcpClient::new(target_addr, size_config);
            udp.test_traffic_load();
        }
        ("client", "udp") => {
            info!("Udp Client");
        }
        ("server", "tcp") => {
            info!("Tcp Server");
            let bind_config_str = CONFIG["load_test"]["target"].as_str().unwrap();
            let bind_config = bind_config_str.parse().unwrap();
            let size_config_integer = CONFIG["load_test"]["packet_size"].as_integer().unwrap();
            let size_config = size_config_integer as usize;
            let tcp = tcp_server::TcpServer::new(bind_config, size_config);
            tcp.test_traffic_load();
        }
        ("server", "udp") => {
            info!("Udp Server");
        }
        _ => error!("Errors in the configuration file"),
    }
}
