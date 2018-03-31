// TODO:
// - Add fragment support
// - Make object expectations fail non-object inputs
#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate graphql_parser;
extern crate gqlog;

use std::{ env, io };
use serde_json::de::{ IoRead };

use gqlog::filter_stream;

fn main() {
    // Get query from input arguments
    let _args: Vec<String> = env::args().collect();
    let default_query = String::from("{}");
    let query = _args.get(1).unwrap_or(&default_query);

    let reader = IoRead::new(io::stdin());
    filter_stream::<IoRead<io::Stdin>>(query.to_string(), reader, |value| {
        println!("{}", value.to_string());
    });
}
