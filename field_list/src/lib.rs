// This file is a big mess but it works, at least for now.

// To debug the macro generation
// cargo rustc --profile=check -- -Zunpretty=expanded
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DataStruct, DeriveInput, Fields, PathArguments};

#[proc_macro_derive(FieldList)]
pub fn derive_field_list(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);

  let fields = match &input.data {
    Data::Struct(DataStruct {
      fields: Fields::Named(fields),
      ..
    }) => &fields.named,
    _ => panic!("expected a struct with named fields"),
  };
  let field_name = fields.iter().map(|field| &field.ident);
  let field_type = fields.iter().map(|field| &field.ty);

  let struct_name = &input.ident;

  // This will contain the code to extracts the value in the proper format for SQL
  // used in `fn value_list`:
  // - quotes for String
  // - no quotes for numbers
  // - NULL for empty options
  // Will silently ignore anything that is not an integer, a String, an Option
  // on an integer or an Option on a String.
  let values = fields.iter().filter_map(|field| {
    let field_type = match field.ty {
      syn::Type::Path(ref typepath) if typepath.qself.is_none() => &typepath.path,
      _ => panic!("Could not extract type path"),
    };
    let type_name = field_type
      .segments
      .last() // last to deal with std::blabla::what::Option
      .unwrap()
      .ident
      .to_string();
    // eprintln!("typename {}", type_name);
    match type_name.as_str() {
      "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" => {
        let field_name = field.ident.as_ref().unwrap();
        Some(quote!(format!("{}", self.#field_name),))
      }
      "String" => {
        let field_name = field.ident.as_ref().unwrap();
        Some(quote!(format!("\"{}\"", self.#field_name),))
      }
      "Option<i8>" | "Option<i16>" | "Option<i32>" | "Option<i64>" | "Option<i128>"
      | "Option<u8>" | "Option<u16>" | "Option<u32>" | "Option<u64>" | "Option<u128>" => {
        let field_name = field.ident.as_ref().unwrap();
        Some(quote!(self.#field_name.map_or("NULL", s => format!("{}", s)),))
      }
      "Option" => {
        // All these shenanigans to fetch the Option's subtype
        if let PathArguments::AngleBracketed(sub_type) = field_type.segments.last().unwrap().arguments.to_owned() {
          if let syn::GenericArgument::Type(sub_type) = sub_type.args.first().unwrap() {
            let sub_type = match sub_type {
              syn::Type::Path(ref typepath) if typepath.qself.is_none() => &typepath.path,
              _ => panic!("Could not extract sybtype path"),
            };
            let sub_type_name = sub_type.segments.last().unwrap().ident.to_string();
              // eprintln!("sub typename {:?}", sub_type_name);
              match sub_type_name.as_str() {
                "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" => {
                  let field_name = field.ident.as_ref().unwrap();
                  Some(quote!(self.#field_name.as_ref().map_or("NULL".to_string(), |s| format!("{}", s)),))
                }
                "String" => {
                  let field_name = field.ident.as_ref().unwrap();
                  Some(quote!(self.#field_name.as_ref().map_or("NULL".to_string(), |s| format!("\"{}\"", s)),))
                }
                _ => None,
              }
            } else {
              None
          }
        } else {
          None
        }
      }
      _ => None,
    }
  });

  // For add
  // TODO: Find a more elegant solution but to clone the iterator
  let field_name2 = field_name.clone();
  let values2 = values.clone();
  // For from_hash
  let field_name3 = field_name.clone();
  // let is_options = vec![
  //   quote!(.unwrap().to_owned()),
  //   quote!(.unwrap().to_owned()),
  //   quote!(.map(|v| v.to_owned())),
  //   quote!(.map(|v| v.to_owned())),
  //   quote!(.map(|v| v.to_owned())),
  //   quote!(.map(|v| v.parse::<i32>().unwrap())),
  //   quote!(.map(|v| v.parse::<u32>().unwrap())),
  //   quote!(.map(|v| v.parse::<u32>().unwrap())),
  // ];
  // let is_options_iter = is_options.iter();

  // Create the specific code for each used in from_hash to convert from the
  // sqlite String to the proper type.
  // TODO: Rework all this duplicated and awful code.
  let is_options_iter = fields.iter().filter_map(|field| {
    let field_type = match field.ty {
      syn::Type::Path(ref typepath) if typepath.qself.is_none() => &typepath.path,
      _ => panic!("Could not extract type path"),
    };
    let type_name = field_type
      .segments
      .last() // last to deal with std::blabla::what::Option
      .unwrap()
      .ident
      .to_string();
    // eprintln!("typename {}", type_name);
    match type_name.as_str() {
      "u8" => Some(quote!(.unwrap().parse::<u8>().unwrap())),
      "u16" => Some(quote!(.unwrap().parse::<u16>().unwrap())),
      "u32" => Some(quote!(.unwrap().parse::<u32>().unwrap())),
      "u64" => Some(quote!(.unwrap().parse::<u64>().unwrap())),
      "u128" => Some(quote!(.unwrap().parse::<u128>().unwrap())),
      "i8" => Some(quote!(.unwrap().parse::<i8>().unwrap())),
      "i16" => Some(quote!(.unwrap().parse::<i16>().unwrap())),
      "i32" => Some(quote!(.unwrap().parse::<i32>().unwrap())),
      "i64" => Some(quote!(.unwrap().parse::<i64>().unwrap())),
      "i128" => Some(quote!(.unwrap().parse::<i128>().unwrap())),
      "String" => Some(quote!(.unwrap().to_owned())),
      "Option" => {
        // All these shenanigans to fetch the Option's subtype
        if let PathArguments::AngleBracketed(sub_type) =
          field_type.segments.last().unwrap().arguments.to_owned()
        {
          if let syn::GenericArgument::Type(sub_type) = sub_type.args.first().unwrap() {
            let sub_type = match sub_type {
              syn::Type::Path(ref typepath) if typepath.qself.is_none() => &typepath.path,
              _ => panic!("Could not extract sybtype path"),
            };
            let sub_type_name = sub_type.segments.last().unwrap().ident.to_string();
            // eprintln!("sub typename {:?}", sub_type_name);
            match sub_type_name.as_str() {
              "u8" => Some(quote!(.map(|v| v.parse::<u8>().unwrap()))),
              "u16" => Some(quote!(.map(|v| v.parse::<u16>().unwrap()))),
              "u32" => Some(quote!(.map(|v| v.parse::<u32>().unwrap()))),
              "u64" => Some(quote!(.map(|v| v.parse::<u64>().unwrap()))),
              "u128" => Some(quote!(.map(|v| v.parse::<u128>().unwrap()))),
              "i8" => Some(quote!(.map(|v| v.parse::<i8>().unwrap()))),
              "i16" => Some(quote!(.map(|v| v.parse::<i16>().unwrap()))),
              "i32" => Some(quote!(.map(|v| v.parse::<i32>().unwrap()))),
              "i64" => Some(quote!(.map(|v| v.parse::<i64>().unwrap()))),
              "i128" => Some(quote!(.map(|v| v.parse::<i128>().unwrap()))),
              "String" => Some(quote!(.map(|v| v.to_owned()))),
              _ => None,
            }
          } else {
            None
          }
        } else {
          None
        }
      }
      _ => None,
    }
  });

  TokenStream::from(quote! {
    use std::collections::HashMap;

    impl #struct_name {
      // TODO: Find a better place for this function. Here it will be replicated
      // in all struct deriving this macro.
      // Performs an arbitrary query on the connection
      fn execute_query(connection: &Connection, query: &str) -> Result<Vec<HashMap<String, String>>> {
        tracing::debug!("query: {}", query);
        let query = query;
        let mut statement = connection.prepare(query)?;
        let mut result: Vec<HashMap<String, String>> = Vec::new();
        while let Ok(State::Row) = statement.next() {
          let column_names = statement.column_names();
          let mut entries = HashMap::new();
          for column_name in column_names {
            if let Ok(value) = statement.read::<String, _>(&**column_name) {
              entries.insert(column_name.to_owned(), value);
            }
          }
          result.push(entries);
        }

        Ok(result)
      }

      pub fn field_list() -> Vec<(String, String)> {
        return vec![#(
          (
            std::stringify!(#field_name).to_string(),
            std::stringify!(#field_type).to_string().replace(" ", ""),
          ),
        )*]
      }

      // Convert rust types to sql types. A limited number of types are accepted.
      fn to_sql_type(field_type: &str) -> Result<String> {
        let result = match field_type {
          "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" => {
            "INTEGER NON NULL"
          }
          "Option<i8>" | "Option<i16>" | "Option<i32>" | "Option<i64>" | "Option<i128>"
          | "Option<u8>" | "Option<u16>" | "Option<u32>" | "Option<u64>" | "Option<u128>" => "INTEGER",
          "String" => "TEXT NON NULL",
          "Option<String>" => "TEXT",
          _ => anyhow::bail!("to_sql_type: {} unknown type conversion to sql", field_type),
        };
        Ok(result.into())
      }

      pub fn create_table(
        connection: &Connection,
        table_name: &str) -> Result<()> {
        let table = #struct_name::field_list()
          .iter()
          .map(|(field_name, field_type)| match #struct_name::to_sql_type(field_type) {
            Ok(field_type) => Ok((field_name.to_string(), field_type.to_string())),
            Err(e) => Err(e),
          })
          // https://doc.rust-lang.org/rust-by-example/error/iter_result.html#fail-the-entire-operation-with-collect
          .collect::<Result<Vec<(String, String)>>>()?
          .iter()
          .map(|(field_name, field_type)| {
            if field_name == "id" {
              format!("{} {} PRIMARY KEY", field_name, field_type)
            } else {
              format!("{} {}", field_name, field_type)
            }
          })
          .collect::<Vec<String>>()
          .join(",");
        #struct_name::execute_query(&connection,
          &format!("CREATE TABLE IF NOT EXISTS {} ({});", table_name, table))?;
        // https://www.sqlite.org/fts3.html#termprefix
        // #struct_name::execute_query(&connection,
        //   &format!("CREATE VIRTUAL TABLE IF NOT EXISTS {} USING fts5({});", table_name, table))?;
        Ok(())
      }

      pub fn value_list(&self) -> Vec<String> {
        return vec![#(#values)*]
      }

      // Look for the entry in the DB, update it if present, create it otherwise. This makes
      // scan reentrant when using an SQL store.
      // The entry type must be Identifiable (have an id field) and Iterable. We will
      // then use struct_iterable to iterate over the field of the type and insert in
      // the database.
      pub fn add(&self, connection: &Connection, table_name: &str) -> Result<()> {
        // Check if the UIDs are not already present in the database
        let constraints = format!("id=\"{}\"", self.id());
        let already_present =
          !Song::execute_query(connection, &format!("SELECT * FROM {} WHERE {};", "songs", constraints))?
            .is_empty();

        let column_names = vec![#(std::stringify!(#field_name2).to_string(),)*];
        let values = vec![#(#values2)*];
        if already_present {
          let sets = column_names.iter().zip(values.iter())
            .map(|(name, value)| format!("{}={}", name, value))
            .collect::<Vec<String>>()
            .join(",");
          let query = &format!("UPDATE {} SET {} WHERE {};", table_name, sets, constraints);
          Song::execute_query(connection, query)?;
        } else {
          // No entry, create a new one
          let column_names = column_names.join(",");
          let values = values.join(",");
          let query = &format!("INSERT INTO {} ({}) VALUES ({});", table_name, column_names, values,);
          Song::execute_query(connection, query)?;
        }

        Ok(())
      }

      fn from_hash(hashmap: &HashMap<String, String>) -> Result<#struct_name> {
        Ok(#struct_name {
          #(
            #field_name3: hashmap.get(std::stringify!(#field_name3))#is_options_iter,
          )*
        })
      }

      pub fn from_sqlite_result(results: &Vec<HashMap<String, String>>) -> Vec<#struct_name> {
        results
          .iter()
          .map(|r| #struct_name::from_hash(r))
          .filter_map(|s| s.ok())
          .collect::<Vec<Song>>()
      }

      pub fn get(connection: &Connection, table_name: &str, id: &str) -> Result<Option<#struct_name>> {
        let result =
           #struct_name::execute_query(&connection, &format!(r#"SELECT * from {} WHERE id="{}";"#, table_name, id))?;
        if !result.is_empty() {
          Ok(Some(#struct_name::from_hash(result.first().unwrap())?))
        } else {
          Ok(None)
        }
      }

      pub fn get_all_with_pagination(
        connection: &Connection,
        table_name: &str,
        page: Option<u32>,
        per_page: Option<u32>,
      ) -> Result<Vec<Song>> {
        let offset: u32;
        let limit: u32;
        if page.is_none() || per_page.is_none() {
          offset = 0;
          limit = u32::MAX;
        } else {
          offset = page.unwrap() * per_page.unwrap();
          limit = per_page.unwrap();
        }
        let result =
           #struct_name::execute_query(
             &connection,
             &format!(r#"SELECT * from {} LIMIT {} OFFSET {};"#, table_name, limit, offset))?;
        Ok(
          result
            .iter()
            .map(|r| #struct_name::from_hash(r))
            .filter_map(|s| s.ok())
            .collect::<Vec<Song>>()
        )
      }

      pub fn get_all(connection: &Connection, table_name: &str) -> Result<Vec<Song>> {
        #struct_name::get_all_with_pagination(connection, table_name, None, None)
      }
    }
  })
}
