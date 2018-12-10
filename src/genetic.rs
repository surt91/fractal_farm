use std::collections::HashMap;

use rocket_contrib::{json::Json,templates::Template};

use diesel::prelude::*;

use super::db::DbConn;

use super::a_fractal_a_day as fractal;
use super::json2fractal;
use super::add_fractal_to_db;
use super::SubmitDetails;

fn combine_fractals(
    f1: &fractal::fractal::Fractal,
    f2: &fractal::fractal::Fractal
)
    -> fractal::fractal::Fractal
{
    f1.combine(f2).expect("failed combining")
}

#[get("/combine/<id1>/<id2>")]
pub fn combine(conn: DbConn, id1: i64, id2: i64) -> Json<SubmitDetails> {
    use schema::fractals;

    // get the two fractals from database
    let json1 = fractals::table.select(fractals::json)
        .find(id1)
        .first::<String>(&*conn)
        .unwrap();
    let json2 = fractals::table.select(fractals::json)
        .find(id2)
        .first::<String>(&*conn)
        .unwrap();

    let f1 = json2fractal(&json1);
    let f2 = json2fractal(&json2);

    let f = combine_fractals(&f1, &f2);

    let (id, high, low) = add_fractal_to_db(&conn, &f.json());

    Json(
        SubmitDetails {
            id,
            low,
            high
        }
    )
}

#[get("/random")]
pub fn random(conn: DbConn) -> String {
    use schema::fractals;
    use diesel::dsl::sql;

    let id = fractals::table.select(fractals::id)
        .filter(fractals::rank.gt(0))
        .limit(1)
        .order(sql::<i64>("RANDOM()"))
        .first::<i64>(&*conn)
        .expect("Error getting random fractals");

    format!("{}", id)
}

#[get("/breed")]
pub fn breed() -> Template {
    let context: HashMap<&str, &str> = HashMap::new();

    Template::render("breed", &context)
}
