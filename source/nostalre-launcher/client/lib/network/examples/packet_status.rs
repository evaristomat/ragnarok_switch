//! Prints the packet status report (`nostalre_network::packet_status`); scripts/packet-status.sh writes it to
//! nostalre-client/docs/packet-status-<packetver>.md.
//!
//! cargo run -p nostalre-network --example packet_status

fn main() {
    // a handler that panics on a filled copy is reported in its row, not on stderr
    std::panic::set_hook(Box::new(|_| {}));
    print!("{}", nostalre_network::packet_status::report());
}
