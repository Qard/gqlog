//! # gqlog
//!
//!  👾 filter your json logs with graphql 👾
//!
//! ## streams
//!
//! ```
//! extern crate serde_json;
//! extern crate gqlog;
//!
//! use serde_json::de::{ StrRead };
//!
//! fn main() {
//!     let query = String::from("{ foo }");
//!     let data = r#"{
//!         "foo": "bar",
//!         "baz": "buz"
//!     }"#;
//!     let reader = StrRead::new(data);
//!
//!     gqlog::filter_stream::<StrRead>(query, reader, |value| {
//!         assert_eq!(value.to_string(), "{\"foo\":\"bar\"}");
//!     });
//! }
//! ```
//!
//! ## serde_json::Value
//!
//! ```
//! #[macro_use] extern crate serde_json;
//! extern crate gqlog;
//!
//! fn main() {
//!     let query = String::from("{ foo }");
//!     let data = json!({
//!         "foo": "bar",
//!         "baz": "buz"
//!     });
//!
//!     assert_eq!(gqlog::filter_value(query, data).to_string(), "{\"foo\":\"bar\"}");
//! }
//! ```
//!
//! ## JSON strings
//!
//! ```
//! extern crate serde_json;
//! extern crate gqlog;
//!
//! fn main() {
//!     let query = String::from("{ foo }");
//!     let data = String::from(r#"{
//!         "foo": "bar",
//!         "baz": "buz"
//!     }"#);
//!
//!     assert_eq!(gqlog::filter(query, data).to_string(), "{\"foo\":\"bar\"}");
//! }
//! ```

// TODO:
// - Add fragment support
// - Make object expectations fail non-object inputs
extern crate serde;
#[allow(unused_imports)]
#[macro_use]
extern crate serde_json;
extern crate graphql_parser;

use serde_json::{ Value };
use serde_json::de::{ Deserializer, Read };
use graphql_parser::parse_query;

mod filters {
    use serde_json::{ Value };
    use serde_json::map::Map;
    use graphql_parser::query::*;
    use graphql_parser::query::Value as GValue;

    #[derive(Clone, Debug)]
    pub enum Filters {
        Field(String),
        Object(String, Vec<Filters>),
        Entries(String, Vec<Filters>),
    }

    // TODO: Fail when requested fields do not exist
    fn filter_object(filters: Vec<Filters>, object: Map<String, Value>) -> Map<String, Value> {
        let mut map = Map::new();

        for item in filters {
            match item {
                Filters::Field(field) => {
                    if let Some(value) = object.get(&field) {
                        // NOTE: Filter fields to support nested arrays
                        map.insert(field, filter_value(Vec::new(), value.clone()));
                    }
                },
                Filters::Object(field, fields) => {
                    if let Some(value) = object.get(&field) {
                        map.insert(field, filter_value(fields, value.clone()));
                    }
                }
                Filters::Entries(field, fields) => {
                    if let Some(value) = object.get(&field) {
                        if let Value::Object(ref items) = *value {
                            let array = filter_array(fields,
                                items.iter().map(|(k, v)| {
                                    Value::Object(vec![
                                        ("key".into(), Value::String(k.clone())),
                                        ("value".into(), v.clone()),
                                    ].into_iter().collect())
                                }).collect());
                            map.insert(field, Value::Array(array));
                        } else {
                            map.insert(field,
                                filter_value(fields, value.clone()));
                        }
                    }
                }
            }
        }

        map
    }

    // TODO: Figure out how to do this without cloning...
    fn filter_array(filters: Vec<Filters>, array: Vec<Value>) -> Vec<Value> {
        array.iter().map(|v| filter_value(filters.clone(), v.clone())).collect()
    }

    pub fn filter_value(filters: Vec<Filters>, data: Value) -> Value {
        match data {
            Value::Object(object) => Value::Object(filter_object(filters, object)),
            Value::Array(array) => Value::Array(filter_array(filters, array)),
            _ => data
        }
    }

    pub fn get_filters(selection: SelectionSet) -> Vec<Filters> {
        selection.items.iter()
            .filter_map(|selection| {
                if let Selection::Field(field) = selection.clone() {
                    let subfilters = get_filters(field.selection_set);
                    if field.arguments.len() > 0 {
                        for argument in &field.arguments {
                            match &argument.0[..] {
                                "entries" => {
                                    match argument.1 {
                                        GValue::Boolean(true) => {
                                            return Some(Filters::Entries(
                                                field.name,
                                                subfilters));
                                        }
                                        GValue::Boolean(false) => {}
                                        _ => {
                                            panic!("invalid argument {:?}",
                                                argument);
                                        }
                                    }
                                }
                                _ => panic!("invalid argument {:?}", argument),
                            }
                        }
                    }
                    if subfilters.len() > 0 {
                        Some(Filters::Object(field.name, subfilters))
                    } else {
                        Some(Filters::Field(field.name))
                    }
                } else {
                    None
                }
            })
            .collect::<Vec<Filters>>()
    }

    pub fn get_selection(ast: Document) -> Vec<Filters> {
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
}

/// Filter a stream of JSON objects and trigger the callback for each.
///
/// # Examples
///
/// ```
/// extern crate serde_json;
/// extern crate gqlog;
///
/// use serde_json::de::{ StrRead };
///
/// fn main() {
///     let query = String::from("{ foo }");
///     let data = r#"{
///         "foo": "bar",
///         "baz": "buz"
///     }"#;
///     let reader = StrRead::new(data);
///
///     gqlog::filter_stream::<StrRead>(query, reader, |value| {
///         println!("{}", value.to_string());
///     });
/// }
/// ```
#[allow(dead_code)]
pub fn filter_stream<'de, R>(query: String, reader: R, func: fn(Value)) where R: Read<'de> {
    // Parse query string to AST
    match parse_query(&query) {
        Err(error) => panic!("Bad query: {}", error),
        Ok(ast) => {
            // Convert AST to selection tree
            let selection = filters::get_selection(ast);

            // Create deserializer stream from stdin
            let stream = Deserializer::new(reader).into_iter::<Value>();

            // For each item in the stream, filter the data
            for value in stream {
                func(filters::filter_value(selection.clone(), value.unwrap()));
            }
        }
    }
}

/// Filter a serde_json::Value.
///
/// # Examples
///
/// ```
/// #[macro_use] extern crate serde_json;
/// extern crate gqlog;
///
/// fn main() {
///     let query = String::from("{ foo }");
///     let data = json!({
///         "foo": "bar",
///         "baz": "buz"
///     });
///
///     assert_eq!(gqlog::filter_value(query, data).to_string(), "{\"foo\":\"bar\"}");
/// }
/// ```
#[allow(dead_code)]
pub fn filter_value(query: String, value: Value) -> Value {
    // Parse query string to AST
    match parse_query(&query) {
        Err(error) => panic!(error),
        Ok(ast) => {
            // Convert AST to selection tree
            let selection = filters::get_selection(ast);

            // For each item in the stream, filter the data
            filters::filter_value(selection, value)
        }
    }
}

/// Filter a json string.
///
/// # Examples
///
/// ```
/// extern crate serde_json;
/// extern crate gqlog;
///
/// fn main() {
///     let query = String::from("{ foo }");
///     let data = String::from(r#"{
///         "foo": "bar",
///         "baz": "buz"
///     }"#);
///
///     println!("{}", gqlog::filter(query, data).to_string());
/// }
/// ```
#[allow(dead_code)]
pub fn filter(query: String, json: String) -> Value {
    // Parse query string to AST
    match parse_query(&query) {
        Err(error) => panic!(error),
        Ok(ast) => {
            // Parse JSON string
            match serde_json::from_str(&json) {
                Err(error) => panic!(error),
                Ok(value) => {
                    // Convert AST to selection tree
                    let selection = filters::get_selection(ast);

                    // For each item in the stream, filter the data
                    filters::filter_value(selection, value)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::de::{ StrRead };

    #[test]
    fn filter_stream() {
        let query = String::from("{ foo }");
        let data = r#"{
            "foo": "bar",
            "baz": "buz"
        }"#;
        let reader = StrRead::new(data);

        super::filter_stream::<StrRead>(query, reader, |value| {
            let expect = r#"{"foo":"bar"}"#;

            assert_eq!(value.to_string(), expect);
        });
    }

    #[test]
    fn filter_value() {
        let query = String::from("{ foo }");
        let data = json!({
            "foo": "bar",
            "baz": "buz"
        });

        let expect = r#"{"foo":"bar"}"#;

        assert_eq!(super::filter_value(query, data).to_string(), expect);
    }

    #[test]
    fn filter() {
        let query = String::from("{ foo }");
        let data = String::from(r#"{
            "foo": "bar",
            "baz": "buz"
        }"#);

        let expect = r#"{"foo":"bar"}"#;

        assert_eq!(super::filter(query, data).to_string(), expect);
    }

    #[test]
    fn nested_objects() {
        let query = String::from("{ nested { foo } }");
        let data = json!({
            "nested": {
                "foo": "bar",
                "baz": "buz"
            }
        });

        let expect = r#"{"nested":{"foo":"bar"}}"#;

        assert_eq!(super::filter_value(query, data).to_string(), expect);
    }

    #[test]
    fn nested_arrays() {
        let query = String::from("{ nested { foo } }");
        let data = json!({
            "nested": [
                {
                    "foo": "bar",
                    "baz": "buz"
                },
                {
                    "foo": "bar",
                    "baz": "buz"
                }
            ]
        });

        let expect = r#"{"nested":[{"foo":"bar"},{"foo":"bar"}]}"#;

        assert_eq!(super::filter_value(query, data).to_string(), expect);
    }

    #[test]
    fn dictionary() {
        let query = String::from("{ dict(entries: true) { value { name } } }");
        let data = json!({
            "dict": {
                "item1": {"name": "item one"},
                "item2": {"name": "item two"},
            },
        });

        let expect = r#"{"dict":[{"value":{"name":"item one"}},{"value":{"name":"item two"}}]}"#;

        assert_eq!(super::filter_value(query, data).to_string(), expect);
    }
}
