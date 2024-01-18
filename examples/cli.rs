#[macro_use]
extern crate log;

use clap::Parser;

/// A demo of ocli with clap
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Config {
    /// Log level
    #[arg(short, long, default_value_t = log::Level::Info)]
    pub log_level: log::Level,
}

fn main() {
    let config = Config::parse();
    ocli::init(config.log_level).unwrap();

    println!("this is onstdout — try to pipe it to another command like `grep` or `wc`");
    error!("log at error level on stderr");
    warn!("log at warn level on stderr");
    info!("log at info level on stderr");
    debug!("log at debug level on stderr");
    trace!("log at trace level on stderr");
    info!("the logs at any level are meant to inform the user");
    info!("while still being able to pipe stdout");
}
