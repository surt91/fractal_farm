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
use rocket::response::{NamedFile, Redirect};
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


#[get("/")]
fn index() -> Redirect {
    Redirect::to("/generate")
}

#[get("/generate")]
fn generate(conn: DbConn) -> Template {
    use models::Fractal;
    use schema::fractals;

    let seed = time::now_utc().to_timespec().sec as usize;

    let f = generate_fractal(seed);
    let json = f.json();

    let old_fractal = fractals::table.order(fractals::rank.desc())
        .first::<Fractal>(&*conn)
        .unwrap_or_else(
            |_| {
                let first = models::NewFractal {
                    json: json.clone(),
                    rank: Some(1)
                };

                diesel::insert_into(fractals::table)
                    .values(&first)
                    .execute(&*conn)
                    .expect("Error saving new entry");

                fractals::table.order(fractals::rank.desc())
                    .first::<Fractal>(&*conn)
                    .expect("Can not find first entry I just saved")
            }
        );

    let new_fractal = models::NewFractal {
        json,
        rank: None
    };

    diesel::insert_into(fractals::table)
        .values(&new_fractal)
        .execute(&*conn)
        .expect("Error saving new entry");

    let new_fractal = fractals::table.order(fractals::created_time.desc())
        .first::<Fractal>(&*conn)
        .expect("Can not find first entry I just saved");


    let mut context: HashMap<&str, &Fractal> = HashMap::new();
    context.insert("agressor", &new_fractal);
    context.insert("defender", &old_fractal);

    Template::render("generate", &context)
}

#[get("/rate/<id>")]
fn rate(conn: DbConn, id: i64) -> Template {
    use models::Fractal;
    use schema::fractals;

    let agressor = fractals::table.find(id)
        .first::<Fractal>(&*conn)
        .expect("the requested id does not exist");

    let opponent_rank = match agressor.rank {
        Some(x) => x - 1,
        None => {
            fractals::table.order(fractals::rank.desc())
                .first::<Fractal>(&*conn)
                .map(|x| x.rank)
                .unwrap_or(None)
                .unwrap_or(1)
        }
    };

    println!("{}", opponent_rank);
    let defender = fractals::table.filter(fractals::rank.eq(opponent_rank))
        .first::<Fractal>(&*conn)
        .expect("the requested id-1 does not exist");

    let mut context: HashMap<&str, &Fractal> = HashMap::new();
    context.insert("agressor", &agressor);
    context.insert("defender", &defender);

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
    winner: i64,
    loser: i64
}

#[post("/rated", data = "<result>")]
fn rated(conn: DbConn, result: Form<DuelResult>) -> Redirect {
    use schema::fractals;
    use models::Fractal;

    let winner = result.get().winner;
    let loser = result.get().loser;

    // the first time, we look at the same thing
    // so assign it rank 1 and generate the next one
    if winner == loser {
        diesel::update(fractals::table.find(winner))
            .set(fractals::rank.eq(1))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> up");

        return Redirect::to("/generate")
    }

    let won_rank = fractals::table.select(fractals::rank)
        .find(loser)
        .first::<Option<i64>>(&*conn)
        .unwrap()
        .unwrap_or(1);

    diesel::update(fractals::table.find(loser))
        .set(fractals::rank.eq::<Option<i64>>(None))
        .execute(&*conn)
        .expect("Error saving new entry: loser -> tmp");
    diesel::update(fractals::table.find(winner))
        .set(fractals::rank.eq(won_rank))
        .execute(&*conn)
        .expect("Error saving new entry: winner -> up");
    diesel::update(fractals::table.find(loser))
        .set(fractals::rank.eq(won_rank + 1))
        .execute(&*conn)
        .expect("Error saving new entry: loser -> down");

    let winner = fractals::table.find(winner)
        .first::<Fractal>(&*conn)
        .unwrap();

    if won_rank == 1 {
        Redirect::to("/generate")
    } else {
        Redirect::to(&format!("/rate/{}", winner.id))
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
fn render_draft(conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
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
        .limit(10)
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
                    consume,
                    archive,
                    generate,
                    rate,
                    rated,
                ]
            )
           .attach(Template::fairing())
           .launch();
}
