#![feature(plugin)]
#![feature(custom_derive)]
#![plugin(rocket_codegen)]

use std::collections::HashMap;

use std::fs;
use std::io::prelude::*;
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

pub mod schema;
pub mod models;

mod db;

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

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/random")]
fn random() -> Template {
    let seed = time::now_utc().to_timespec().sec as usize;
    let filename = basename2path(&format!("{}", seed));

    let mut json_path = filename.clone();
    json_path.set_extension("json");

    let fractal_type = fractal::FractalType::MobiusFlame;

    let size = 512;
    let dim = (size, size);

    // hacky do while loop
    let mut fractal = fractal::fractal::FractalBuilder::new()
                                     .seed(seed)
                                     .style(&None)
                                     .variation(&None)
                                     .symmetry(&None)
                                     .vibrancy(&None)
                                     .gamma(&None)
                                     .build(&fractal_type);

    let json = fractal::fractal::render_draft(&mut fractal, filename.to_str().unwrap(), &dim);

    let mut file = fs::File::create(json_path).unwrap();
    write!(&mut file, "{}", json).unwrap();

    let seed_str = &format!("{}", seed);

    let mut context: HashMap<&str, &str> = HashMap::new();
    context.insert("path", filename.to_str().unwrap());
    context.insert("json", &json);
    context.insert("seed", seed_str);

    Template::render("random", &context)
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

use models::NewFractal;
#[post("/grade", data = "<user_input>")]
fn grade(conn: DbConn, user_input: Form<NewFractal>) -> Redirect {
    use schema::fractals;

    println!("{:?}", user_input.get());

    diesel::insert_into(fractals::table)
        .values(user_input.get())
        .execute(&*conn)
        .expect("Error saving new entry");

    Redirect::to("/random")
}

#[get("/render/<id>/<width>/<height>")]
fn render(conn: DbConn, id: i64, width: u32, height: u32) -> Redirect {
    use models::Fractal;
    use schema::fractals;
    // use schema::fractals::dsl::*;

    let f: Fractal = fractals::table.find(id)
        .first::<Fractal>(&*conn)
        .unwrap();

    let dim = (width, height);
    let path = json2png(&f.json, dim);
    let path = path.to_str().unwrap();
    Redirect::to(&format!("/{}", path))
}

#[get("/top")]
fn top(conn: DbConn) -> Template {
    use schema::fractals;
    use models::Fractal;
    use schema::fractals::dsl::*;

    let pngs: Vec<Fractal> = fractals.order(fractals::score.desc())
        .limit(10)
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
                    random,
                    files,
                    list,
                    grade,
                    top,
                    render
                ]
            )
           .attach(Template::fairing())
           .launch();
}
