#[macro_use]
extern crate log;

fn main() {
    ocli::init(log::Level::Trace).unwrap();

    error!("This is printed to stderr, with the 'path(line): error: ' prefix colored in red");
    warn!("This is printed to stderr, with the 'path(line): warn: ' prefix colored in yellow");
    info!("This is printed to stderr, with the 'path(line): info: ' prefix");
    debug!("This is printed to stderr, with the 'path(line): debug: ' prefix colored in blue");
    trace!("This is printed to stderr, with the 'path(line): trace: ' prefix colored in magenta");
}
