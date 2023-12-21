use std::collections::HashMap;
use std::fs;
use std::io;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::{
  http::{header, StatusCode, Uri},
  response::IntoResponse,
  routing::get,
  Json, Router,
};
use axum_macros;
use base64ct::{Base64, Encoding};
use clap::Parser;
use id3::{Tag, TagLike};
use jwalk::WalkDir;
use md5::Digest;
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use sqlite::{Connection, ConnectionThreadSafe, State};
use struct_iterable::Iterable;
use tower_http::{
  services::ServeDir,
  trace::{self, TraceLayer},
};
use tracing::Level;
use tracing_subscriber;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about = None)]
struct Config {
  /// Scan path for mp3s to be added to the database
  #[arg(short = 's', long, value_name = "PATH")]
  scan_path: Option<PathBuf>,
  /// Database
  #[arg(short = 'd', long, default_value = PathBuf::from("rstream.db").into_os_string(), value_name = "PATH")]
  database: String,
  /// Address to listen to
  #[arg(short = 'o', long, default_value = "127.0.0.1")]
  host: Ipv4Addr,
  /// Port to listen to
  #[arg(short = 'p', long, default_value = "3000")]
  port: u16,
  /// Scan only
  #[arg(short = 'c', long, default_value = "false")]
  scan_only: bool,
  /// Static assets folder
  #[arg(short = 'a', long, value_name = "PATH")]
  static_assets_folder: Option<String>,
  /// Log level
  #[arg(short = 'l', long, default_value = "info")]
  log_level: Level,
}

trait Identifiable {
  fn id(&self) -> &String;
}

#[derive(Debug, Iterable, Serialize, Deserialize)]
struct Song {
  id: String,
  path: String,
  title: String,
  artist: String,
  album: String,
  year: String,
  // comment: String,
  track: String,
  speed: String,
  genre_str: String,
  start_time: String,
  end_time: String,
}

impl Identifiable for Song {
  fn id(&self) -> &String {
    return &self.id;
  }
}

impl Default for Song {
  fn default() -> Song {
    Song {
      id: "".to_string(),
      path: "".to_string(),
      title: "".to_string(),
      artist: "".to_string(),
      album: "".to_string(),
      year: "".to_string(),
      // comment: "".to_string(),
      track: "".to_string(),
      speed: "".to_string(),
      genre_str: "".to_string(),
      start_time: "".to_string(),
      end_time: "".to_string(),
    }
  }
}

// There is some \0 in some songs tags, filter them
fn clean_string(s: &str) -> String {
  s.replace(|c: char| (c as u8) < 32, "")
    // https://www.sqlite.org/faq.html#q14
    .replace("\"", "\"\"")
    .replace("\'", "\'\'")
    .to_string()
}

impl Song {
  pub fn from_tags(path: &Path, tag: &id3::Tag) -> Result<Song> {
    let title =
      if let Some(title) = tag.title() { clean_string(title) } else { "<No title>".into() };
    let artist =
      if let Some(artist) = tag.artist() { clean_string(artist) } else { "Unknown".into() };
    let album = if let Some(album) = tag.album() { clean_string(album) } else { "".into() };
    let year = if let Some(year) = tag.year() { year.to_string() } else { "".into() };
    // let comment =
    //   if let Some(comment) = tag.comment() { clean_string(comment) } else { "No comment".into() };

    Ok(Song {
      id: md5sum(&path)?,
      path: path.to_string_lossy().to_string(),
      title,
      artist,
      album,
      year,
      // comment,
      ..Default::default()
    })
  }
}

fn prepare_db(connection: &Connection, table_name: &str, fields: &[&str]) -> Result<()> {
  let table = fields
    .iter()
    .map(|s| s.to_string() + " TEXT NON NULL")
    .collect::<Vec<String>>()
    .join(",");
  connection.execute(format!("CREATE TABLE IF NOT EXISTS {} ({});", table_name, table))?;
  Ok(())
}

fn md5sum(path: &Path) -> Result<String> {
  let mut file = fs::File::open(&path)?;
  let mut hasher = md5::Md5::new();
  let _n = io::copy(&mut file, &mut hasher)?;
  return Ok(Base64::encode_string(&hasher.finalize()));
}

// Performs an arbitrary query on the connection
fn execute_query(connection: &Connection, query: &str) -> Result<Vec<HashMap<String, String>>> {
  let query = query;
  // println!("query {}", query);
  let mut statement = connection.prepare(query)?;
  let mut result: Vec<HashMap<String, String>> = Vec::new();
  while let Ok(State::Row) = statement.next() {
    let column_names = statement.column_names();
    let mut entries = HashMap::new();
    for column_name in column_names {
      entries.insert(column_name.to_owned(), statement.read::<String, _>(&**column_name)?);
    }
    result.push(entries);
  }

  Ok(result)
}

// Look for the entry in the DB, update it if present, create it otherwise. This makes
// scan reentrant when using an SQL store.
// The entry type must be Identifiable (have an id field) and Iterable. We will
// then use struct_iterable to iterate over the field of the type and insert in
// the database.
fn add_song<T: Iterable + Identifiable>(
  connection: &Connection,
  table_name: &str,
  entry: &T,
) -> Result<()> {
  // Check if the UIDs are not already present in the database
  let constraints = format!("id=\"{}\"", entry.id());
  let already_present =
    !execute_query(connection, &format!("SELECT * FROM {} WHERE {};", "songs", constraints))?
      .is_empty();

  if already_present {
    // The entry already exists, update it
    let sets = entry
      .iter()
      .filter(|(field, _)| field != &"id")
      .map(|(field, value)| {
        format!(
          "{}=\"{}\"",
          // TODO: get rid of unwrap
          field,
          value.downcast_ref::<String>().unwrap(),
        )
      })
      .collect::<Vec<String>>()
      .join(",");
    let query = &format!("UPDATE {} SET {} WHERE {};", table_name, sets, constraints);
    execute_query(connection, query)?;
  } else {
    // No entry, create a new one
    let column_names = entry
      .iter()
      .map(|(field, _)| field)
      .collect::<Vec<&str>>()
      .join(",");
    let values = entry
      .iter()
      .map(|(_, value)| value.downcast_ref::<String>().unwrap().clone())
      .map(|value| format!("\"{}\"", value)) // enclose in ""
      .collect::<Vec<String>>()
      .join(",");
    let query = &format!("INSERT INTO {} ({}) VALUES ({});", table_name, column_names, values,);
    execute_query(connection, query)?;
  }
  Ok(())
}

fn truncate(s: &str, max_chars: usize) -> String {
  match s.char_indices().nth(max_chars) {
    None => s.to_string(),
    Some((idx, _)) => format!("{}...", &s[..idx]),
  }
}

// Scans the provided folder for all files and for every file which has id3 tags
// compute a md5 hash and create (or update, or do nothing) en entry in the
// database
fn scan(data_path: &Path, config: &Config) -> Result<()> {
  let connection = Connection::open(&config.database)?;
  let song = Song {
    ..Default::default()
  };
  // We create a table containing all the fields of the struct we want to store.
  // The type we iterate on must be struct_iterable::Iterable.
  let fields = song.iter().map(|(field, _)| field).collect::<Vec<&str>>();
  let _ = prepare_db(&connection, "songs", &fields);

  let on_a_tty = atty::is(atty::Stream::Stdout);
  let mut file_count = 0;
  connection.execute("BEGIN TRANSACTION;")?;
  for entry in WalkDir::new(data_path) {
    let entry = entry?;
    let path = entry.path();
    if !path.is_dir() {
      if let Ok(tag) = Tag::read_from_path(&path) {
        file_count += 1;
        if on_a_tty {
          let filename = path
            .file_name()
            .ok_or(anyhow::anyhow!("Not a file"))?
            .to_string_lossy();
          print!("{}{: >8} {}", "\r\x1b[2K", file_count, truncate(&filename, 80));
          std::io::stdout().flush()?;
        }
        let song = Song::from_tags(&path, &tag)?;
        add_song(&connection, "songs", &song)?;
      }
    }
  }
  connection.execute("END TRANSACTION;")?;

  println!("{}{} file(s) parsed", "\r\x1b[2K", file_count);
  Ok(())
}

async fn root() -> &'static str {
  concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

#[axum_macros::debug_handler]
async fn get_song(
  axum::extract::Path(song_id): axum::extract::Path<String>,
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
) -> impl IntoResponse {
  if let Ok(result) =
    execute_query(&connection, &format!(r#"SELECT * from songs WHERE id="{}""#, song_id))
  {
    if !result.is_empty() {
      Json(result.first()).into_response()
    } else {
      StatusCode::NOT_FOUND.into_response()
    }
  } else {
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
  }
}

#[axum_macros::debug_handler]
async fn get_songs(
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
) -> impl IntoResponse {
  if let Ok(result) = execute_query(&connection, "SELECT * from songs") {
    Json(result).into_response()
  } else {
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
  }
}

// Embed static web site
#[derive(RustEmbed)]
#[folder = "assets/"]
struct Asset;

async fn static_handler(uri: Uri) -> impl IntoResponse {
  let mut path = uri.path().trim_start_matches('/').to_string();
  // Files are embedded without the containing folder path
  if path.starts_with("assets/") {
    path = path.replace("assets/", "");
  }

  match Asset::get(path.as_str()) {
    Some(content) => {
      let mime = mime_guess::from_path(path).first_or_octet_stream();
      ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
    }
    None => StatusCode::NOT_FOUND.into_response(),
  }
}

async fn serve(config: &Config) -> Result<()> {
  let connection = Arc::new(Connection::open_thread_safe(&config.database)?);
  let nb_songs = if let Ok(result) = execute_query(&connection, "SELECT * from songs") {
    result.len()
  } else {
    eprintln!("error: {} database if not properly formatted. No songs found!", config.database);
    eprintln!("rescan you music folder with:");
    eprintln!("  rstream --scan-path /path/to/music");
    anyhow::bail!("Incorrectly formatted database")
  };

  // Build our application with a route
  let mut app = Router::new()
    .route("/", get(root))
    .route("/songs", get(get_songs))
    .route("/songs/:song_id", get(get_song))
    .with_state(Arc::clone(&connection));

  // Either serve static files from a provided folder or from the embedded
  // static files from the assets folder
  if let Some(ref static_assets_folder) = config.static_assets_folder {
    app = app.nest_service("/assets", ServeDir::new(&static_assets_folder));
    tracing::debug!("serving static assets at {}", static_assets_folder);
  } else {
    // We use a wildcard matcher ("/assets/*file") to match against everything
    // within our defined assets directory.
    app = app.route("/assets/*file", get(static_handler));
    tracing::debug!("serving embedded assets");
  }

  // Add some logging on each request/response
  app = app.layer(
    TraceLayer::new_for_http()
      .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
      .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
  );

  // run our app with hyper
  let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
    .await
    .unwrap();
  tracing::debug!("listening on {}", listener.local_addr().unwrap());
  println!(
    "serving {} songs from {} on {}...",
    nb_songs,
    config.database,
    listener.local_addr().unwrap()
  );
  axum::serve(listener, app).await.unwrap();
  Ok(())
}

#[tokio::main]
async fn main() {
  let config = Config::parse();

  // Configure a custom event formatter
  let format = tracing_subscriber::fmt::format()
    .with_level(true)
    .with_target(false)
    .compact();
  let subscriber = tracing_subscriber::fmt()
    .event_format(format)
    .with_max_level(config.log_level)
    .finish();
  tracing::subscriber::set_global_default(subscriber).unwrap();

  if let Some(ref scan_path) = config.scan_path {
    scan(&scan_path, &config).unwrap();
  }
  if !config.scan_only {
    if let Err(_) = serve(&config).await {
      std::process::exit(1);
    }
  }
}
