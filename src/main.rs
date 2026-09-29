use std::collections::HashMap;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use dotenv::dotenv;

use a_fractal_a_day as fractal;

use axum::Router;
use axum::extract::{DefaultBodyLimit, FromRef, Path, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::Json;
use tower_http::services::ServeDir;

#[macro_use] extern crate diesel;
use diesel::prelude::*;
use serde::Serialize;

mod db;

mod db_convenience;
pub mod schema;
pub mod models;

use db::{DbConn, Pool};

mod rating;
mod genetic;

const MAX: i64 = 100;

#[derive(Clone)]
pub struct AppState {
    pool: Pool,
    tera: Arc<tera::Tera>,
}

impl FromRef<AppState> for Pool {
    fn from_ref(state: &AppState) -> Pool {
        state.pool.clone()
    }
}

impl FromRef<AppState> for Arc<tera::Tera> {
    fn from_ref(state: &AppState) -> Arc<tera::Tera> {
        state.tera.clone()
    }
}

/// Runs blocking work (database, rendering fractals) outside of the async
/// runtime. A panic inside is answered with `InternalServerError`.
pub async fn blocking<F, T>(f: F) -> Result<T, StatusCode>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

/// Renders `templates/<name>.html.tera`.
pub fn render_template<C: Serialize>(tera: &tera::Tera, name: &str, context: &C) -> Result<Html<String>, StatusCode> {
    tera::Context::from_serialize(context)
        .and_then(|ctx| tera.render(name, &ctx))
        .map(Html)
        .map_err(|e| {
            eprintln!("rendering template {} failed: {:?}", name, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

/// Loads all `templates/*.html.tera` under their name without extension,
/// e.g. `base`, so that `{% extends "base" %}` keeps working.
fn init_templates() -> tera::Tera {
    let files: Vec<(PathBuf, Option<String>)> = fs::read_dir("templates")
        .expect("templates directory")
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            let name = path.file_name()?.to_str()?.strip_suffix(".html.tera")?.to_owned();
            Some((path, Some(name)))
        })
        .collect();

    let mut tera = tera::Tera::default();
    tera.add_template_files(files).expect("templates");
    tera
}

fn sha2(input: &str) -> String {
    use sha2::{Sha256, Digest};

    let mut hasher = Sha256::default();
    hasher.input(input.as_bytes());
    let output = hasher.result();

    format!("{:X}", output)
}

fn basename2path(name: &str) -> PathBuf {
    let mut filename = PathBuf::from("./fractals");
    fs::create_dir_all(&filename).unwrap();
    filename.push(name);
    filename.set_extension("png");

    filename
}

fn json2fractal(json: &str) -> fractal::fractal::Fractal {
    let fractal_type = fractal::FractalType::LoadJson(json.to_owned());

    fractal::fractal::FractalBuilder::new()
        .build(&fractal_type)
}

fn json2png(json: &str, dim: (u32, u32)) -> PathBuf {
    let filename = format!("{}x{}_{}", dim.0, dim.1, sha2(json));
    let path = basename2path(&filename);

    if path.exists() {
        return path
    }

    let mut fractal = json2fractal(json);

    fractal::fractal::render_wrapper(&mut fractal, path.to_str().unwrap(), &dim, false);

    path
}

fn json2draft(json: &str, dim: (u32, u32)) -> PathBuf {
    let filename = format!("d_{}x{}_{}", dim.0, dim.1, sha2(json));
    let path = basename2path(&filename);

    if path.exists() {
        return path
    }

    let mut fractal = json2fractal(json);

    fractal::fractal::render_draft(&mut fractal, path.to_str().unwrap(), &dim);

    path
}

// the rendered pngs are served by the `/fractals` file service
fn png_url(path: &std::path::Path) -> String {
    format!("/fractals/{}", path.file_name().unwrap().to_str().unwrap())
}

fn generate_fractal(seed: usize, name: Option<fractal::FractalType>) -> fractal::fractal::Fractal {
    let fractal_type = match name {
        Some(x) => x,
        None => match seed % 6 {
            0 => fractal::FractalType::MobiusFlame,
            1 => fractal::FractalType::FractalFlame,
            2 => fractal::FractalType::RandomLSystem,
            3 => fractal::FractalType::Mandelbrot,
            4 => fractal::FractalType::Newton,
            5 => fractal::FractalType::QuadraticMap,
            _ => unreachable!()
        }
    };

    let mut ctr = 0;
    let fractal = loop {
        let mut f = fractal::fractal::FractalBuilder::new()
            .seed(seed + ctr)
            .build(&fractal_type);

        ctr += 1;
        // try to generate an interesting fractal
        if f.estimate_quality_before() {
            break f
        }
    };

    fractal
}

fn add_fractal_to_db(conn: &mut DbConn, json: &str) -> (i64, i64, i64) {
    use models::Fractal;
    use schema::fractals;

    // special case of empty database, add this fractal with rank 1
    if let None = fractals::table.order(fractals::rank.desc())
        .filter(fractals::rank.le(MAX))
        .first::<Fractal>(&mut **conn)
        .ok()
    {
        let first = models::NewFractal {
            json: json.to_owned(),
            rank: Some(1)
        };

        diesel::insert_into(fractals::table)
            .values(&first)
            .execute(&mut **conn)
            .expect("Error saving new entry");
    }

    let new_fractal = models::NewFractal {
        json: json.to_owned(),
        rank: None
    };

    diesel::insert_into(fractals::table)
        .values(&new_fractal)
        .execute(&mut **conn)
        .expect("Error saving new entry");

    let new_id = fractals::table.select(fractals::id)
        .order(fractals::created_time.desc())
        .first::<i64>(&mut **conn)
        .expect("Can not find first entry I just saved");

    let high = fractals::table.select(diesel::dsl::min(fractals::rank))
        .first::<Option<i64>>(&mut **conn)
        .unwrap()
        .unwrap_or(1);
    let low = fractals::table.select(diesel::dsl::max(fractals::rank))
        .first::<Option<i64>>(&mut **conn)
        .unwrap()
        .unwrap_or(1);

    (new_id, high, low)
}

fn cleanup_db(conn: &mut DbConn) {
    use schema::fractals;

    diesel::delete(
        fractals::table
            .filter(fractals::consumed.eq(false))
            .filter(fractals::deleted.eq(false))
            .filter(fractals::rank.is_null())
    )
    .execute(&mut **conn)
    .expect("Error cleaning up");
}

async fn index() -> Redirect {
    Redirect::to("/generate")
}

async fn generate() -> Redirect {
    Redirect::to("/generate/random")
}

async fn generate_specific(mut conn: DbConn, Path(name): Path<String>) -> Result<Redirect, StatusCode> {
    blocking(move || {
        let seed = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as usize;

        let fractal_type = match name.as_str() {
            "newton" => Some(fractal::FractalType::Newton),
            "mobius" => Some(fractal::FractalType::MobiusFlame),
            "flame" => Some(fractal::FractalType::FractalFlame),
            "qmap" => Some(fractal::FractalType::QuadraticMap),
            "lsys" => Some(fractal::FractalType::RandomLSystem),
            "mandelbrot" => Some(fractal::FractalType::Mandelbrot),
            _ => None
        };

        let f = generate_fractal(seed, fractal_type);
        let json = f.json();

        let (new_id, high, low) = add_fractal_to_db(&mut conn, &json);

        Redirect::to(&format!("/rate/{}/{}/{}", new_id, high, low))
    }).await
}

async fn list(mut conn: DbConn) -> Result<Json<Vec<models::Fractal>>, StatusCode> {
    blocking(move || {
        use schema::fractals::dsl::*;
        use schema::fractals;
        use models::Fractal;

        fractals.order(fractals::id.desc())
            .load::<Fractal>(&mut *conn)
            .map(|x| Json(x))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }).await?
}

async fn render(mut conn: DbConn, Path((id, width, height)): Path<(i64, u32, u32)>) -> Result<Redirect, StatusCode> {
    blocking(move || {
        use models::Fractal;
        use schema::fractals;

        let f: Fractal = fractals::table.find(id)
            .first::<Fractal>(&mut *conn)
            .unwrap();

        let dim = (width, height);
        let path = json2png(&f.json, dim);
        Redirect::to(&png_url(&path))
    }).await
}

async fn draft(mut conn: DbConn, Path((id, width, height)): Path<(i64, u32, u32)>) -> Result<Redirect, StatusCode> {
    blocking(move || {
        use models::Fractal;
        use schema::fractals;

        let f: Fractal = fractals::table.find(id)
            .first::<Fractal>(&mut *conn)
            .unwrap();

        let dim = (width, height);
        let path = json2draft(&f.json, dim);
        Redirect::to(&png_url(&path))
    }).await
}

async fn json(mut conn: DbConn, Path(id): Path<i64>) -> Result<Response, StatusCode> {
    blocking(move || {
        use schema::fractals;

        fractals::table.select(fractals::json)
            .find(id)
            .first::<String>(&mut *conn)
            .ok()
            .map(|x| ([(header::CONTENT_TYPE, "application/json")], x).into_response())
            .ok_or(StatusCode::NOT_FOUND)
    }).await?
}

#[derive(Serialize)]
pub struct SubmitDetails {
    pub id: i64,
    pub low: i64,
    pub high: i64,
}

const LIMIT: usize = 1024*1024*5;

async fn submit_json(mut conn: DbConn, data: String) -> Result<Json<SubmitDetails>, StatusCode> {
    blocking(move || {
        let (id, high, low) = add_fractal_to_db(&mut conn, &data);
        Json(
            SubmitDetails {
                id,
                low,
                high
            }
        )
    }).await
}

async fn upload_json(State(tera): State<Arc<tera::Tera>>) -> Result<Html<String>, StatusCode> {
    let context: HashMap<&str, &str> = HashMap::new();

    render_template(&tera, "uploadJson", &context)
}

async fn consume(mut conn: DbConn) -> Result<String, StatusCode> {
    blocking(move || {
        use models::Fractal;
        use schema::fractals::dsl::*;

        // before we consume: clean up the database
        // this is a good place, since it will be called regulary
        cleanup_db(&mut conn);

        let f: Fractal = fractals
            .filter(rank.gt(0))
            .filter(consumed.eq(false))
            .order(rank.asc())
            .first::<Fractal>(&mut *conn)
            .unwrap();
        // FIXME: if all fractals are consumed: handel the error

        diesel::update(fractals.find(f.id))
            .set((
                consumed.eq(true),
                consumed_time.eq(time::OffsetDateTime::now_utc().unix_timestamp()),
                rank.eq::<Option<i64>>(None),
            ))
            .execute(&mut *conn)
            .expect("Error saving new entry");


        let max_rank = fractals.select(diesel::dsl::max(rank))
            .first::<Option<i64>>(&mut *conn)
            .unwrap()
            .unwrap_or(1);

        db_convenience::offset_rank(&mut conn, 2, max_rank, -1);

        f.json
    }).await
}

async fn top(mut conn: DbConn, State(tera): State<Arc<tera::Tera>>) -> Result<Html<String>, StatusCode> {
    let pngs = blocking(move || {
        use schema::fractals;
        use models::Fractal;
        use schema::fractals::dsl::*;

        fractals.order(fractals::rank.asc())
            .filter(rank.gt(0))
            .filter(consumed.eq(false))
            .filter(deleted.eq(false))
            .limit(MAX)
            .load::<Fractal>(&mut *conn)
            .unwrap()
    }).await?;

    let mut context: HashMap<&str, &Vec<models::Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    render_template(&tera, "top", &context)
}

async fn archive(mut conn: DbConn, State(tera): State<Arc<tera::Tera>>) -> Result<Html<String>, StatusCode> {
    let pngs = blocking(move || {
        use schema::fractals;
        use models::Fractal;
        use schema::fractals::dsl::*;

        fractals.order(fractals::consumed_time.desc())
            .filter(consumed.eq(true))
            .filter(deleted.eq(false))
            .load::<Fractal>(&mut *conn)
            .unwrap()
    }).await?;

    let mut context: HashMap<&str, &Vec<models::Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    render_template(&tera, "top", &context)
}

async fn trash(mut conn: DbConn, State(tera): State<Arc<tera::Tera>>) -> Result<Html<String>, StatusCode> {
    let pngs = blocking(move || {
        use schema::fractals;
        use models::Fractal;
        use schema::fractals::dsl::*;

        fractals.order(fractals::deleted_time.desc())
            .filter(consumed.eq(false))
            .filter(deleted.eq(true))
            .load::<Fractal>(&mut *conn)
            .unwrap()
    }).await?;

    let mut context: HashMap<&str, &Vec<models::Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    render_template(&tera, "top", &context)
}

async fn delete(mut conn: DbConn, Path(id_in): Path<i64>) -> Result<Redirect, StatusCode> {
    blocking(move || {
        use schema::fractals::dsl::*;

        let rank_in = fractals.select(rank)
            .find(id_in)
            .first::<Option<i64>>(&mut *conn)
            .expect("Can not find the rank")
            .expect("rank is None");

        diesel::update(fractals.find(id_in))
            .set((
                deleted.eq(true),
                deleted_time.eq(time::OffsetDateTime::now_utc().unix_timestamp()),
                rank.eq::<Option<i64>>(None),
            ))
            .execute(&mut *conn)
            .expect("Error deleting entry");

        println!("deleted rank {}", rank_in);
        db_convenience::offset_rank(&mut conn, rank_in, MAX, -1);

        Redirect::to("/top")
    }).await
}

async fn editor(mut conn: DbConn, State(tera): State<Arc<tera::Tera>>, Path(id): Path<i64>) -> Result<Html<String>, StatusCode> {
    let json = blocking(move || {
        use schema::fractals;

        fractals::table.select(fractals::json)
            .find(id)
            .first::<String>(&mut *conn)
            .ok()
    }).await?;

    let id_str = format!("{}", id);

    match json {
        Some(j) => {
            let mut context: HashMap<&str, &str> = HashMap::new();
            context.insert("json", &j);
            context.insert("id", &id_str);

            render_template(&tera, "editor", &context)
        }
        None => Err(StatusCode::NOT_FOUND)
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let state = AppState {
        pool: db::init_pool(),
        tera: Arc::new(init_templates()),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/list", get(list))
        .route("/top", get(top))
        .route("/archive", get(archive))
        .route("/trash", get(trash))
        .route("/render/{id}/{width}/{height}", get(render))
        .route("/draft/{id}/{width}/{height}", get(draft))
        .route("/json/{id}", get(json))
        .route("/consume", get(consume))
        .route("/generate", get(generate))
        .route("/generate/{name}", get(generate_specific))
        .route("/delete/{id}", get(delete))
        .route("/rate/{id}/{high}/{low}", get(rating::rate))
        .route("/above", post(rating::above))
        .route("/below", post(rating::below))
        .route("/editor/{id}", get(editor))
        .route("/submitJson", get(upload_json).post(submit_json))
        .route("/combine/{id1}/{id2}", get(genetic::combine))
        .route("/random", get(genetic::random))
        .route("/breed", get(genetic::breed))
        .nest_service("/static", ServeDir::new("static"))
        .nest_service("/fractals", ServeDir::new("fractals"))
        .layer(DefaultBodyLimit::max(LIMIT))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:7878")
        .await
        .expect("bind 0.0.0.0:7878");
    axum::serve(listener, app)
        .await
        .expect("server error");
}
