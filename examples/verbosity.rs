#[macro_use]
extern crate log;

use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};

/// A demo of ocli with clap
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Log level
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

fn main() {
    let config = Config::parse();
    if let Some(level) = config.verbose.log_level() {
        ocli::init(level).unwrap();
    }
    println!("this is on stdout — try to pipe it to another command like `grep` or `wc`");
    error!("log at error level on stderr");
    warn!("log at warn level on stderr");
    info!("log at info level on stderr");
    debug!("log at debug level on stderr");
    trace!("log at trace level on stderr");
    info!("the logs at any level are meant to inform the user");
    info!("while still being able to pipe stdout");
}
