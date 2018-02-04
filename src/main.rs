#![feature(plugin)]
#![plugin(rocket_codegen)]

use std::collections::HashMap;

use std::fs;
use std::io::prelude::*;
extern crate time;
use std::path::{Path, PathBuf};

extern crate rand;

extern crate dotenv;
use dotenv::dotenv;
use std::env;

extern crate a_fractal_a_day;
use a_fractal_a_day as fractal;

extern crate rocket;
use rocket::response::NamedFile;
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

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/random")]
fn random() -> Template {
    let seed = time::now_utc().to_timespec().sec as usize;
    let mut filename = PathBuf::from("./fractals");
    fs::create_dir_all(&filename).unwrap();
    filename.push(&format!("{}", seed));
    filename.set_extension("png");

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

    let (finished, description, json)
        = fractal::fractal::render_wrapper(&mut fractal, filename.to_str().unwrap(), &dim, false);

    let mut file = fs::File::create(json_path).unwrap();
    let json = json.replace("\\n", "\n");
    let json = json.replace("\\\"", "\"");
    write!(&mut file, "{}", json).unwrap();

    let seed_str = &format!("{}", seed);

    let mut context: HashMap<&str, &str> = HashMap::new();
    context.insert("path", filename.to_str().unwrap());
    context.insert("description", &description);
    context.insert("json", &json);
    context.insert("finished", if finished {"good"} else {"bad"});
    context.insert("seed", seed_str);

    Template::render("random", &context)
}

#[get("/list")]
fn list(conn: DbConn) -> QueryResult<Json<Vec<models::Fractal>>> {
// fn list(conn: DbConn) {
    use schema::fractals::dsl::*;
    use schema::fractals;
    use models::Fractal;

    fractals.order(fractals::id.desc())
        .load::<Fractal>(&*conn)
        .map(|x| Json(x))
}

#[get("/<file..>")]
fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(".").join(file)).ok()
}

fn main() {
    dotenv().ok();

    rocket::ignite()
           .manage(db::init_pool())
           .mount("/", routes![index, random, files, list])
           .attach(Template::fairing())
           .launch();
}
