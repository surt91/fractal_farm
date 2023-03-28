// for rocket
#![feature(proc_macro_hygiene, decl_macro)]

use std::collections::HashMap;

use std::fs;
extern crate time;
use std::path::{Path, PathBuf};

extern crate rand;
extern crate sha2;

extern crate dotenv;
use dotenv::dotenv;

extern crate a_fractal_a_day;
use a_fractal_a_day as fractal;

#[macro_use] extern crate rocket;
use rocket::response::{NamedFile, Redirect, content};

#[macro_use] extern crate diesel;
use diesel::prelude::*;
extern crate r2d2;
#[macro_use] extern crate serde_derive;

extern crate serde;
extern crate serde_json;

mod db;

extern crate rocket_contrib;
use rocket_contrib::{json::Json,templates::Template};

mod db_convenience;
pub mod schema;
pub mod models;

use db::DbConn;

mod rating;
mod genetic;

const MAX: i64 = 100;

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

#[get("/")]
fn index() -> Redirect {
    Redirect::to(uri!(generate))
}

#[get("/generate")]
fn generate() -> Redirect {
    Redirect::to(uri!(generate_specific: "random"))
}

#[get("/generate/<name>")]
fn generate_specific(mut conn: DbConn, name: Option<String>) -> Redirect {
    let seed = time::OffsetDateTime::now_utc().unix_timestamp_nanos() as usize;

    let fractal_type = match name.as_ref().map(|s| s.as_str()) {
        Some("newton") => Some(fractal::FractalType::Newton),
        Some("mobius") => Some(fractal::FractalType::MobiusFlame),
        Some("flame") => Some(fractal::FractalType::FractalFlame),
        Some("qmap") => Some(fractal::FractalType::QuadraticMap),
        Some("lsys") => Some(fractal::FractalType::RandomLSystem),
        Some("mandelbrot") => Some(fractal::FractalType::Mandelbrot),
        None | Some(_) => None
    };

    let f = generate_fractal(seed, fractal_type);
    let json = f.json();

    let (new_id, high, low) = add_fractal_to_db(&mut conn, &json);

    Redirect::to(uri!(rating::rate: new_id, high, low))
}

#[get("/list")]
fn list(mut conn: DbConn) -> QueryResult<Json<Vec<models::Fractal>>> {
    use schema::fractals::dsl::*;
    use schema::fractals;
    use models::Fractal;

    fractals.order(fractals::id.desc())
        .load::<Fractal>(&mut *conn)
        .map(|x| Json(x))
}

#[get("/render/<id>/<width>/<height>")]
fn render(mut conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
    use models::Fractal;
    use schema::fractals;

    let f: Fractal = fractals::table.find(id)
        .first::<Fractal>(&mut *conn)
        .unwrap();

    let dim = (width, height);
    let path = json2png(&f.json, dim);
    let path = path.to_str().unwrap();
    Redirect::to(uri!(files: path))
}

#[get("/draft/<id>/<width>/<height>")]
fn draft(mut conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
    use models::Fractal;
    use schema::fractals;

    let f: Fractal = fractals::table.find(id)
        .first::<Fractal>(&mut *conn)
        .unwrap();

    let dim = (width, height);
    let path = json2draft(&f.json, dim);
    let path = path.to_str().unwrap();
    Redirect::to(uri!(files: path))
}

#[get("/json/<id>")]
fn json(mut conn: DbConn, id: i64) -> Option<content::Json<String>> {
    use schema::fractals;

    fractals::table.select(fractals::json)
        .find(id)
        .first::<String>(&mut *conn)
        .ok()
        .and_then(|x| Some(content::Json(x)))
}

#[derive(Serialize)]
pub struct SubmitDetails {
    pub id: i64,
    pub low: i64,
    pub high: i64,
}

struct JsonFractal {
    data: String
}

// https://api.rocket.rs/v0.4/rocket/data/trait.FromDataSimple.html
use std::io::Read;
use rocket::{Request, Data, Outcome::*};
use rocket::data::{self, FromDataSimple};
use rocket::http::Status;
const LIMIT: u64 = 1024*1024*5;
impl FromDataSimple for JsonFractal {
    type Error = String;

    fn from_data(_req: &Request, data: Data) -> data::Outcome<Self, String> {
        // Read the data into a String.
        let mut string = String::new();
        if let Err(e) = data.open().take(LIMIT).read_to_string(&mut string) {
            return Failure((Status::InternalServerError, format!("{:?}", e)));
        }
        Success(JsonFractal { data: string })
    }
}

#[post("/submitJson", data = "<json>")]
fn submit_json(mut conn: DbConn, json: JsonFractal) -> Json<SubmitDetails> {
    let (id, high, low) = add_fractal_to_db(&mut conn, &json.data);
    Json(
        SubmitDetails {
            id,
            low,
            high
        }
    )
}
#[get("/submitJson")]
fn upload_json() -> Template {
    let context: HashMap<&str, &str> = HashMap::new();

    Template::render("uploadJson", &context)
}

#[get("/consume")]
fn consume(mut conn: DbConn) -> String {
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
}

#[get("/top")]
fn top(mut conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::rank.asc())
        .filter(rank.gt(0))
        .filter(consumed.eq(false))
        .filter(deleted.eq(false))
        .limit(MAX)
        .load::<Fractal>(&mut *conn)
        .unwrap();

    let mut context: HashMap<&str, &Vec<Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    Template::render("top", &context)
}

#[get("/archive")]
fn archive(mut conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::consumed_time.desc())
        .filter(consumed.eq(true))
        .filter(deleted.eq(false))
        .load::<Fractal>(&mut *conn)
        .unwrap();

    let mut context: HashMap<&str, &Vec<Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    Template::render("top", &context)
}

#[get("/trash")]
fn trash(mut conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::deleted_time.desc())
        .filter(consumed.eq(false))
        .filter(deleted.eq(true))
        .load::<Fractal>(&mut *conn)
        .unwrap();

    let mut context: HashMap<&str, &Vec<Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    Template::render("top", &context)
}

#[get("/delete/<id_in>")]
fn delete(mut conn: DbConn, id_in: i64) -> Redirect {
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

    Redirect::to(uri!(top))
}

#[get("/editor/<id>")]
fn editor(mut conn: DbConn, id: i64) -> Option<Template> {
    use schema::fractals;

    let json = fractals::table.select(fractals::json)
        .find(id)
        .first::<String>(&mut *conn)
        .ok();

    let id_str = format!("{}", id);

    match json {
        Some(j) => {
            let mut context: HashMap<&str, &str> = HashMap::new();
            context.insert("json", &j);
            context.insert("id", &id_str);

            Some(Template::render("editor", &context))
        }
        None => None
    }
}

#[get("/<file..>", rank = 2)]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(".").join(file)).ok()
}

fn main() {
    dotenv().ok();

    rocket::ignite()
           .manage(db::init_pool())
           .mount("/",
                routes![
                    index,
                    files,
                    list,
                    top,
                    archive,
                    trash,
                    render,
                    draft,
                    json,
                    consume,
                    generate,
                    generate_specific,
                    delete,
                    rating::rate,
                    rating::above,
                    rating::below,
                    editor,
                    submit_json,
                    upload_json,
                    genetic::combine,
                    genetic::random,
                    genetic::breed,
                ]
            )
           .attach(Template::fairing())
           .launch();
}
