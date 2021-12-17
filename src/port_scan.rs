use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::tcp::{self, MutableTcpPacket, TcpFlags};
use pnet::transport::{
    self, TransportChannelType, TransportProtocol, TransportReceiver, TransportSender,
};
impl PortScan {
    enum ScanType{
        Syn = TcpFlags::SYN as isize,
        Fin = TcpFlags::FIN as isize,
        Xmas = (TcpFlags::FIN | TcpFlags::URG | TcpFlags::PSH) as isize,
        Null = 0,
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
