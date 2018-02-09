#![feature(plugin)]
#![feature(custom_derive)]
#![plugin(rocket_codegen)]

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

extern crate rocket;
use rocket::response::{NamedFile, Redirect, content};
use rocket::request::Form;

extern crate rocket_contrib;
use rocket_contrib::{Json,Template};

#[macro_use] extern crate diesel;
use diesel::prelude::*;
extern crate r2d2_diesel;
extern crate r2d2;

#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

mod db;
pub mod schema;
pub mod models;

use db::DbConn;

const MAX: i64 = 10;

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

fn json2png(json: &str, dim: (u32, u32)) -> PathBuf {
    let filename = format!("{}x{}_{}", dim.0, dim.1, sha2(json));
    let path = basename2path(&filename);

    if path.exists() {
        return path
    }

    let fractal_type = fractal::FractalType::LoadJson(json.to_owned());

    let mut fractal = fractal::fractal::FractalBuilder::new()
                              .build(&fractal_type);

    fractal::fractal::render_wrapper(&mut fractal, path.to_str().unwrap(), &dim, false);

    path
}

fn json2draft(json: &str, dim: (u32, u32)) -> PathBuf {
    let filename = format!("d_{}x{}_{}", dim.0, dim.1, sha2(json));
    let path = basename2path(&filename);

    if path.exists() {
        return path
    }

    let fractal_type = fractal::FractalType::LoadJson(json.to_owned());

    let mut fractal = fractal::fractal::FractalBuilder::new()
                              .build(&fractal_type);

    fractal::fractal::render_draft(&mut fractal, path.to_str().unwrap(), &dim);

    path
}

fn generate_fractal(seed: usize) -> fractal::fractal::Fractal {
    let fractal_type = fractal::FractalType::MobiusFlame;

    let fractal = fractal::fractal::FractalBuilder::new()
        .seed(seed)
        .style(&None)
        .variation(&None)
        .symmetry(&None)
        .vibrancy(&None)
        .gamma(&None)
        .build(&fractal_type);

    fractal
}

fn add_fractal_to_db(conn: DbConn, json: &str) -> Redirect {
    use models::Fractal;
    use schema::fractals;

    // special case of empty database, add this fractal with rank 1
    if let None = fractals::table.order(fractals::rank.desc())
        .filter(fractals::rank.le(MAX))
        .first::<Fractal>(&*conn)
        .ok()
    {
        let first = models::NewFractal {
            json: json.to_owned(),
            rank: Some(1)
        };

        diesel::insert_into(fractals::table)
            .values(&first)
            .execute(&*conn)
            .expect("Error saving new entry");
    }

    let new_fractal = models::NewFractal {
        json: json.to_owned(),
        rank: None
    };

    diesel::insert_into(fractals::table)
        .values(&new_fractal)
        .execute(&*conn)
        .expect("Error saving new entry");

    let new_id = fractals::table.select(fractals::id)
        .order(fractals::created_time.desc())
        .first::<i64>(&*conn)
        .expect("Can not find first entry I just saved");

    let high = fractals::table.select(diesel::dsl::min(fractals::rank))
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(1);
    let low = fractals::table.select(diesel::dsl::max(fractals::rank))
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(1);

    Redirect::to(&format!("/rate/{}/{}/{}", new_id, high, low))
}


#[get("/")]
fn index() -> Redirect {
    Redirect::to("/generate")
}

#[get("/generate")]
fn generate(conn: DbConn) -> Redirect {
    let seed = time::now_utc().to_timespec().sec as usize;

    let f = generate_fractal(seed);
    let json = f.json();

    add_fractal_to_db(conn, &json)
}

#[get("/rate/<id>/<high>/<low>")]
fn rate(conn: DbConn, id: i64, high: i64, low: i64) -> Template {
    use schema::fractals;

    let candidate_rank = fractals::table.select(fractals::rank)
        .find(id)
        .first::<Option<i64>>(&*conn)
        .unwrap();

    // if the candidate is new (no rank) start with the worst ranked fractal
    let opponent_rank = match candidate_rank {
        Some(_) => (low + high) / 2,
        None => low
    };

    println!("low {}", low);
    println!("high {}", high);
    println!("opp rank {}", opponent_rank);
    let pivot_id = fractals::table.select(fractals::id)
        .filter(fractals::rank.eq(opponent_rank))
        .first::<i64>(&*conn)
        .expect("the requested rank does not exist");

    let mut context: HashMap<&str, i64> = HashMap::new();
    context.insert("agressor", id);
    context.insert("defender", pivot_id);
    context.insert("high", high);
    context.insert("low", low);

    Template::render("generate", &context)
}

#[get("/list")]
fn list(conn: DbConn) -> QueryResult<Json<Vec<models::Fractal>>> {
    use schema::fractals::dsl::*;
    use schema::fractals;
    use models::Fractal;

    fractals.order(fractals::id.desc())
        .load::<Fractal>(&*conn)
        .map(|x| Json(x))
}

#[derive(FromForm)]
struct DuelResult {
    candidate: i64,
    pivot: i64,
    low: i64,
    high: i64,
}

#[post("/below", data = "<result>")]
fn below(conn: DbConn, result: Form<DuelResult>) -> Redirect {
    use schema::fractals;

    let pivot = result.get().pivot;
    let candidate = result.get().candidate;
    // let high = result.get().high;
    let low = result.get().low;

    // the first time, we look at the same thing
    // so assign it rank 1 and generate the next one
    if pivot == candidate {
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq(1))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> up");

        return Redirect::to("/generate")
    }

    let pivot_rank = fractals::table.select(fractals::rank)
        .find(pivot)
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(1);

    let candidate_rank = fractals::table.select(fractals::rank)
        .find(candidate)
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(MAX + 1);

    println!("below: candidate id: {}", candidate);
    println!("below: pivot: {}", pivot_rank);
    println!("below: candidate: {}", candidate_rank);

    if pivot_rank == MAX || candidate_rank == MAX + 1 {
        // remove candidate from database
        diesel::delete(fractals::table.find(candidate))
            .execute(&*conn)
            .expect("Error deleting posts");

        return Redirect::to("/generate")
    }

    if pivot > candidate {
        // set the candidate rank to NULL
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> null");

        // move all up, between the current rank and the pivot rank
        // diesel::update(
        //         fractals::table.filter(fractals::rank.le(pivot_rank))
        //             .filter(fractals::rank.gt(candidate_rank))
        //     )
        //     .set(fractals::rank.eq(fractals::rank - 1))
        //     .execute(&*conn)
        //     .expect("Error saving new entry: make space");
        // FIXME: there has to be a way to do is thin one query
        // TODO write SQL by hand?
        for i in (pivot_rank..candidate_rank).rev() {
            println!("{}", i);
            diesel::update(
                    fractals::table.filter(fractals::rank.eq(i))
                )
                .set(fractals::rank.eq(fractals::rank - 1))
                .execute(&*conn)
                .expect("Error saving new entry: make space");
        }

        // insert candidate into the empty spot of the pivot
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq(pivot_rank))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> pivot");

        // limit to MAX
        diesel::update(fractals::table.filter(fractals::rank.gt(MAX)))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: limit to MAX");
    }

    let high = pivot_rank + 1;

    if high == low {
        Redirect::to("/generate")
    } else {
        Redirect::to(&format!("/rate/{}/{}/{}", candidate, high, low))
    }
}

#[post("/above", data = "<result>")]
fn above(conn: DbConn, result: Form<DuelResult>) -> Redirect {
    use schema::fractals;

    let pivot = result.get().pivot;
    let candidate = result.get().candidate;
    let high = result.get().high;
    // let low = result.get().low;

    // the first time, we look at the same thing
    // so assign it rank 1 and generate the next one
    if pivot == candidate {
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq(1))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> up");

        return Redirect::to("/generate")
    }

    let pivot_rank = fractals::table.select(fractals::rank)
        .find(pivot)
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap();

    let candidate_rank = fractals::table.select(fractals::rank)
        .find(candidate)
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(MAX + 1);

    println!("above: pivot: {}", pivot_rank);
    println!("above: candidate: {}", candidate_rank);

    if pivot < candidate {
        // set the candidate rank to NULL
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> null");


        // move all down, between the current rank and the pivot rank
        // diesel::update(
        //         fractals::table.filter(fractals::rank.ge(pivot_rank))
        //             .filter(fractals::rank.lt(candidate_rank))
        //     )
        //     .set(fractals::rank.eq(fractals::rank + 1))
        //     .execute(&*conn)
        //     .expect("Error saving new entry: make space");
        // FIXME: there has to be a way to do is thin one query
        // TODO write SQL by hand?
        for i in (pivot_rank..candidate_rank).rev() {
            println!("{}", i);
            diesel::update(
                    fractals::table.filter(fractals::rank.eq(i))
                )
                .set(fractals::rank.eq(fractals::rank + 1))
                .execute(&*conn)
                .expect("Error saving new entry: make space");
        }

        // insert candidate into the empty spot of the pivot
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq(pivot_rank))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> pivot");

        // limit to MAX
        diesel::update(fractals::table.filter(fractals::rank.gt(MAX)))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: limit to MAX");
    }

    let low = pivot_rank;
    println!("{} {}", low, high);

    if high == low {
        Redirect::to("/generate")
    } else {
        Redirect::to(&format!("/rate/{}/{}/{}", candidate, high, low))
    }
}

#[get("/render/<id>/<width>/<height>")]
fn render(conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
    use models::Fractal;
    use schema::fractals;

    let f: Fractal = fractals::table.find(id)
        .first::<Fractal>(&*conn)
        .unwrap();

    let dim = (width, height);
    let path = json2png(&f.json, dim);
    let path = path.to_str().unwrap();
    Redirect::to(&format!("/{}", path))
}

#[get("/draft/<id>/<width>/<height>")]
fn draft(conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
    use models::Fractal;
    use schema::fractals;

    let f: Fractal = fractals::table.find(id)
        .first::<Fractal>(&*conn)
        .unwrap();

    let dim = (width, height);
    let path = json2draft(&f.json, dim);
    let path = path.to_str().unwrap();
    Redirect::to(&format!("/{}", path))
}

#[get("/json/<id>")]
fn json(conn: DbConn, id: i64) -> Option<content::Json<String>> {
    use schema::fractals;

    fractals::table.select(fractals::json)
        .find(id)
        .first::<String>(&*conn)
        .ok()
        .and_then(|x| Some(content::Json(x)))
}

#[post("/sumbitJson", data = "<json>")]
fn submit_json(conn: DbConn, json: String) -> Redirect {
    add_fractal_to_db(conn, &json)
}

#[get("/consume")]
fn consume(conn: DbConn) -> String {
    use models::Fractal;
    use schema::fractals::dsl::*;

    let f: Fractal = fractals
        .filter(rank.gt(0))
        .filter(consumed.eq(false))
        .order(rank.asc())
        .first::<Fractal>(&*conn)
        .unwrap();
    // FIXME: if all fractals are consumed: handel the error

    diesel::update(fractals.find(f.id))
        .set(consumed.eq(true))
        .execute(&*conn)
        .expect("Error saving new entry");

    f.json
}

#[get("/top")]
fn top(conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::rank.asc())
        .filter(rank.gt(0))
        .filter(consumed.eq(false))
        .limit(MAX)
        .load::<Fractal>(&*conn)
        .unwrap();

    let mut context: HashMap<&str, &Vec<Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    Template::render("top", &context)
}

#[get("/archive")]
fn archive(conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::rank.asc())
        .filter(rank.gt(0))
        .filter(consumed.eq(true))
        .load::<Fractal>(&*conn)
        .unwrap();

    let mut context: HashMap<&str, &Vec<Fractal>> = HashMap::new();
    context.insert("pngs", &pngs);

    Template::render("top", &context)
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
                    render,
                    draft,
                    json,
                    consume,
                    archive,
                    generate,
                    rate,
                    above,
                    below
                ]
            )
           .attach(Template::fairing())
           .launch();
}
