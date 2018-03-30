// TODO:
// - Add fragment support
// - Make object expectations fail non-object inputs
extern crate serde_json;
extern crate graphql_parser;

use std::{ env, io };

use serde_json::{ Value };
use serde_json::de::{ Deserializer, IoRead };
use serde_json::map::Map;

use graphql_parser::parse_query;
use graphql_parser::query::*;

#[derive(Clone, Debug)]
enum Filters {
    Field(String),
    Object(String, Vec<Filters>)
}

// TODO: Fail when requested fields do not exist
fn filter_object(filters: Vec<Filters>, object: Map<String, Value>) -> Map<String, Value> {
    let mut map = Map::new();

    for item in filters {
        match item {
            Filters::Field(field) => {
                if let Some(value) = object.get(&field) {
                    // NOTE: Filter fields to support nested arrays
                    map.insert(field, filter(Vec::new(), value.clone()));
                }
            },
            Filters::Object(field, fields) => {
                if let Some(value) = object.get(&field) {
                    map.insert(field, filter(fields, value.clone()));
                }
            }
        }
    }

    map
}

// TODO: Figure out how to do this without cloning...
fn filter_array(filters: Vec<Filters>, array: Vec<Value>) -> Vec<Value> {
    array.iter().map(|v| filter(filters.clone(), v.clone())).collect()
}

fn filter(filters: Vec<Filters>, data: Value) -> Value {
    match data {
        Value::Object(object) => Value::Object(filter_object(filters, object)),
        Value::Array(array) => Value::Array(filter_array(filters, array)),
        _ => data
    }
}

fn get_filters(selection: SelectionSet) -> Vec<Filters> {
    selection.items.iter()
        .filter_map(|selection| {
            if let Selection::Field(field) = selection.clone() {
                if field.selection_set.items.len() > 0 {
                    Some(Filters::Object(field.name, get_filters(field.selection_set)))
                } else {
                    Some(Filters::Field(field.name))
                }
            } else {
                None
            }
        })
        .collect::<Vec<Filters>>()
}

fn get_selection(ast: Document) -> Vec<Filters> {
    // Just do nothing if there are no definitions
    // TODO: Should not having a definition be an input error?
    if let Some(def) = ast.definitions.first() {
        // Only Operation is a valid type here--fragments are discarded
        if let Definition::Operation(op_def) = def.clone() {
            // Only SelectionSet and Query are valid query types,
            // Mutation and Subscription are discarded
            let maybe_selection_set = match op_def {
                OperationDefinition::SelectionSet(selection) => Some(selection),
                OperationDefinition::Query(query) => Some(query.selection_set),
                _ => None
            };

            // Add filters for selection set to the keys, if we found something
            if let Some(selection_set) = maybe_selection_set {
                return get_filters(selection_set)
            }
        }
    }

    // If it got here, we could not find a selection set
    Vec::new()
}

fn main() {
    // Get query from input arguments
    let _args: Vec<String> = env::args().collect();
    let default_query = String::from("{}");
    let query = _args.get(1).unwrap_or(&default_query);

    // Parse query string to AST
    match parse_query(query) {
        Err(error) => panic!(error),
        Ok(ast) => {
            // Convert AST to selection tree
            let selection = get_selection(ast);

            // Create deserializer stream from stdin
            let reader = IoRead::new(io::stdin());
            let stream = Deserializer::new(reader).into_iter::<Value>();

            // For each item in the stream, filter the data
            for value in stream {
                println!("{}", filter(selection.clone(), value.unwrap()).to_string());
            }
        }
    }
}
