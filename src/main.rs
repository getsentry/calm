extern crate clap;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_yaml;
extern crate serde_json;
#[macro_use] extern crate error_chain;
extern crate sha1;
extern crate dotenv;
extern crate indicatif;
extern crate console;
extern crate crossbeam;
extern crate regex;
extern crate glob;
extern crate git2;
extern crate parking_lot;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate if_chain;

mod macros;
mod prelude;

mod cli;
mod config;
mod ctx;
mod errors;
mod report;
mod rt;
mod tools;
mod utils;


fn main() {
    dotenv::dotenv().ok();
    cli::main();
}
