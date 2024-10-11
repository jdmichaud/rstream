use std::fs;
use std::io;
use std::io::{Read, Write};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use axum::{
  http::{header, StatusCode, Uri},
  response::{IntoResponse, Redirect},
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

use field_list::FieldList;

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
  #[arg(short = 'H', long, default_value = "127.0.0.1")]
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
  /// Do not use a database transaction during scanning (slower)
  #[arg(short = 't', long, default_value = "false")]
  do_not_use_transaction: bool,
}

trait Identifiable {
  fn id(&self) -> &String;
}

#[derive(Debug, Iterable, Serialize, Deserialize, FieldList)]
struct Song {
  id: String,
  path: String,
  title: Option<String>,
  artist: Option<String>,
  album: Option<String>,
  year: Option<i32>,
  // comment: String,
  track: Option<u32>,
  disc: Option<u32>,
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
      title: None,
      artist: None,
      album: None,
      year: None,
      // comment: "".to_string(),
      track: None,
      disc: None,
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
    Ok(Song {
      id: md5sum(&path)?,
      path: path.to_string_lossy().to_string(),
      title: tag.title().map(|s| clean_string(s)),
      artist: tag.artist().map(|s| clean_string(s)),
      album: tag.album().map(|s| clean_string(s)),
      year: tag.year(),
      // comment,
      track: tag.track(),
      disc: tag.disc(),
      ..Default::default()
    })
  }
}

fn md5sum(path: &Path) -> Result<String> {
  let mut file = fs::File::open(&path)?;
  let mut hasher = md5::Md5::new();
  let _n = io::copy(&mut file, &mut hasher)?;
  return Ok(Base64::encode_string(&hasher.finalize()));
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

  // We create a table containing all the fields of the struct we want to store.
  // The type we iterate on must be struct_iterable::Iterable.
  Song::create_table(&connection, "songs")?;

  let on_a_tty = atty::is(atty::Stream::Stdout);
  let mut file_count = 0;
  if !config.do_not_use_transaction {
    connection.execute("BEGIN TRANSACTION;")?;
  }
  for entry in WalkDir::new(data_path) {
    let entry = entry?;
    let path = entry.path();
    if !path.is_dir() {
      match Tag::read_from_path(&path) {
        Ok(tag) => {
          file_count += 1;
          if on_a_tty {
            let filename = path
              .file_name()
              .ok_or(anyhow::anyhow!("Not a file"))?
              .to_string_lossy();
            print!("{}{} {}", "\r\x1b[2K", file_count, truncate(&filename, 80));
            std::io::stdout().flush()?;
          }
          let song = Song::from_tags(&path, &tag)?;
          song.add(&connection, "songs")?;
        }
        Err(e) => tracing::debug!("error reading {} id3 tags ({})", path.display(), e),
      }
    }
  }
  if !config.do_not_use_transaction {
    connection.execute("END TRANSACTION;")?;
  }

  println!("{}{} file(s) parsed", "\r\x1b[2K", file_count);
  Ok(())
}

async fn version() -> &'static str {
  concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Deserialize)]
struct Pagination {
  page: Option<u32>,
  per_page: Option<u32>,
}

#[axum_macros::debug_handler]
async fn get_song(
  axum::extract::Path(song_id): axum::extract::Path<String>,
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
) -> impl IntoResponse {
  match Song::get(&connection, "songs", &song_id) {
    Ok(Some(song)) => Json(song).into_response(),
    Ok(None) => StatusCode::NOT_FOUND.into_response(),
    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
  }
}

#[axum_macros::debug_handler]
async fn get_song_file(
  axum::extract::Path(song_id): axum::extract::Path<String>,
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
) -> impl IntoResponse {
  match Song::get(&connection, "songs", &song_id) {
    Ok(Some(song)) => match fs::File::open(&song.path) {
      Ok(f) => {
        let mut reader = std::io::BufReader::new(f);
        let mut buffer = Vec::new();
        let _ = reader.read_to_end(&mut buffer);

        let mime = mime_guess::from_path(song.path).first_or_octet_stream();
        ([(header::CONTENT_TYPE, mime.as_ref())], Into::<axum::body::Body>::into(buffer))
          .into_response()
      }
      Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    },
    Ok(None) => StatusCode::NOT_FOUND.into_response(),
    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
  }
}

#[axum_macros::debug_handler]
async fn get_songs(
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
  pagination: axum::extract::Query<Pagination>,
) -> impl IntoResponse {
  let page = pagination.0.page;
  let per_page = pagination.0.per_page;
  match Song::get_all_with_pagination(&connection, "songs", page, per_page) {
    Ok(songs) => Json(songs).into_response(),
    Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
  }
}

#[derive(Debug, Deserialize)]
struct SearchParams {
  term: String,
}

// Performs an arbitrary query on the connection
fn execute_query(connection: &Connection, query: &str) -> Result<Vec<HashMap<String, String>>> {
  tracing::debug!("query: {:?}", query);
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

fn get_offset_and_limit(pagination: &Pagination) -> (u32, u32) {
  let page = pagination.page;
  let per_page = pagination.per_page;
  // TODO: This logic is already in field_list::lib.rs
  let offset: u32;
  let limit: u32;
  if page.is_none() || per_page.is_none() {
    offset = 0;
    limit = u32::MAX;
  } else {
    offset = page.unwrap() * per_page.unwrap();
    limit = per_page.unwrap();
  }
  (offset, limit)
}

#[axum_macros::debug_handler]
async fn get_albums(
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
  pagination: axum::extract::Query<Pagination>,
) -> impl IntoResponse {
  let (offset, limit) = get_offset_and_limit(&pagination.0);
  match execute_query(
    &connection,
    &format!(
      r#"SELECT DISTINCT(album), artist, year, COUNT(*) as nbsongs FROM songs WHERE LENGTH(album) > 0 GROUP BY album;"#
    ),
  ) {
    Ok(results) => {
      let offset: usize = std::cmp::min(offset as usize, results.len());
      let limit: usize = std::cmp::min(limit as usize, results.len() - offset as usize);
      return Json(&results[offset..offset + limit]).into_response();
    }
    Err(e) => tracing::error!("search failed with {}", e),
  }
  return StatusCode::INTERNAL_SERVER_ERROR.into_response();
}

#[axum_macros::debug_handler]
async fn get_artists(
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
  pagination: axum::extract::Query<Pagination>,
) -> impl IntoResponse {
  let (offset, limit) = get_offset_and_limit(&pagination.0);
  match execute_query(
    &connection,
    &format!(
      r#"SELECT artist, COUNT(*) AS nbsongs FROM songs WHERE LENGTH(artist) > 0 GROUP BY artist;"#
    ),
  ) {
    Ok(results) => {
      let offset: usize = std::cmp::min(offset as usize, results.len());
      let limit: usize = std::cmp::min(limit as usize, results.len() - offset as usize);
      return Json(&results[offset..offset + limit]).into_response();
    }
    Err(e) => tracing::error!("search failed with {}", e),
  }
  return StatusCode::INTERNAL_SERVER_ERROR.into_response();
}

#[axum_macros::debug_handler]
async fn search(
  axum::extract::State(connection): axum::extract::State<Arc<ConnectionThreadSafe>>,
  search_params: axum::extract::Query<SearchParams>,
  pagination: axum::extract::Query<Pagination>,
) -> impl IntoResponse {
  let term = search_params.0.term;
  let (offset, limit) = get_offset_and_limit(&pagination.0);
  match execute_query(
    &connection,
    &format!(
      r#"SELECT * FROM songs WHERE songs MATCH "{}" ORDER BY rank LIMIT {} OFFSET {};"#,
      term, limit, offset
    ),
  ) {
    Ok(results) => {
      println!("{:?}", results);
      let songs = Song::from_sqlite_result(&results);
      return Json(songs).into_response();
    }
    Err(e) => tracing::error!("search failed with {}", e),
  }
  return StatusCode::INTERNAL_SERVER_ERROR.into_response();
}

// Embed static web site
#[derive(RustEmbed)]
#[folder = "assets/"]
struct Asset;

#[axum_macros::debug_handler]
async fn static_handler(uri: Uri) -> impl IntoResponse {
  let mut path = uri.path().trim_start_matches('/').to_string();
  // Files are embedded without the containing folder path
  if path.starts_with("assets/") {
    path = path.replace("assets/", "");
  }

  if path == "" {
    path = "index.html".to_string();
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
  let nb_songs = match Song::get_all(&connection, "songs") {
    Ok(result) => result.len(),
    Err(e) => {
      eprintln!("error: {} database is not properly formatted ({:?})", config.database, e);
      eprintln!("rescan you music folder with:");
      eprintln!("  rstream --scan-path /path/to/music");
      anyhow::bail!("Incorrectly formatted database")
    }
  };

  // Build our application with a route
  let mut app = Router::new()
    .route("/", get(|| async { Redirect::permanent("/assets") }))
    .route("/assets", get(|| async { Redirect::permanent("/assets/index.html") }))
    .route("/assets/", get(|| async { Redirect::permanent("/assets/index.html") }))
    .route("/version", get(version))
    .route("/songs", get(get_songs))
    .route("/songs/:song_id", get(get_song))
    // FIXME: find better URL
    .route("/song/:song_id", get(get_song_file))
    .route("/artists", get(get_artists))
    .route("/albums", get(get_albums))
    .route("/search", get(search))
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
      .on_response(trace::DefaultOnResponse::new().level(Level::INFO).include_headers(true)),
  );

  // run our app with hyper
  let listener = tokio::net::TcpListener::bind(format!("{}:{}", config.host, config.port))
    .await
    .unwrap();
  tracing::debug!("listening on {}", listener.local_addr().unwrap());
  println!(
    "serving {} songs from {} on http://{}...",
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
