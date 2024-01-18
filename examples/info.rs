#[macro_use]
extern crate log;

fn main() {
    ocli::init(log::Level::Info).unwrap();

    error!("This is printed to stderr, with the 'error: ' prefix colored in red");
    warn!("This is printed to stderr, with the 'warn: ' prefix colored in yellow");
    info!("This is printed to stderr, without prefix or color");
    debug!("This is not printed");
    trace!("This is not printed");
}
