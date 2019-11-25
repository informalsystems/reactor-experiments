mod crypto;
mod encoding;
mod events;
mod messages;
mod remote_peer;
mod seed_node;
mod types;

use log::{info, Level};
use simple_logger;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about = "An attempt to demonstrate the ergonomics of single-threaded concurrent code")]
struct Opt {
    /// The host to which to bind.
    #[structopt(short, long, default_value = "127.0.0.1")]
    host: String,

    /// The port to which to bind.
    #[structopt(short, long, default_value = "29000")]
    port: u16,

    /// Increase output logging to DEBUG level.
    #[structopt(short, long)]
    verbose: bool,
}

fn main() -> std::io::Result<()> {
    let opt = Opt::from_args();
    let config = seed_node::SeedNodeConfig {
        bind_host: opt.host,
        bind_port: opt.port,
    };
    simple_logger::init_with_level(if opt.verbose {
        Level::Debug
    } else {
        Level::Info
    })
    .unwrap();
    info!("Binding node to {}:{}", &config.bind_host, &config.bind_port);
    let mut node = seed_node::SeedNode::new(config);
    node.run()
}