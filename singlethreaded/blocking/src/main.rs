mod encoding;
mod events;
mod messages;
mod pubsub;
mod remote_peer;
mod seed_node;
mod types;

use log::{info, Level};
use simple_logger;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about = "An attempt to demonstrate the ergonomics of single-threaded concurrent code")]
struct Opt {
    #[structopt(short, long, default_value = "abcd")]
    id: String,

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
        id: opt.id,
        bind_host: opt.host,
        bind_port: opt.port,
    };
    let log_level = if opt.verbose {
        Level::Debug
    } else {
        Level::Info
    };
    simple_logger::init_with_level(log_level).unwrap();

    info!(
        "Binding node to {}:{}",
        &config.bind_host, &config.bind_port
    );
    let mut node = seed_node::SeedNode::new(config)?;
    // execute the node runtime
    node.run()
}
