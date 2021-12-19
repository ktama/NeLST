use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use pnet::transport::{
    self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender,
};
impl PortScan {
    const TCP_SIZE: usize = 20;

    enum ScanType{
        Syn = TcpFlags::SYN as isize,
        Fin = TcpFlags::FIN as isize,
        Xmas = (TcpFlags::FIN | TcpFlags::URG | TcpFlags::PSH) as isize,
        Null = 0,
    }
    
    struct PacketInfo {
        my_ipaddr: Ipv4Addr,
        target_ipaddr: Ipv4Addr,
        my_port: u16,
        maximum_port: u16,
        scan_type: ScanType,
    }

    pub fn scan(scanType :ScanType) {
        match scanType {
            ScanType.Syn => syn_scan()
    }

    fn tcp_scan() {}
    fn syn_scan() {}
    fn fin_scan() {}
    fn xmas_scan() {}
    fn null_scan() {}
    fn udp_scan() {}

}
