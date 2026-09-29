use std::collections::HashMap;

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Html;
use axum::Json;
use diesel::sql_types::Numeric;

use diesel::prelude::*;

use crate::db::DbConn;

use crate::fractal;
use crate::json2fractal;
use crate::add_fractal_to_db;
use crate::SubmitDetails;
use crate::{blocking, render_template};

fn combine_fractals(
    f1: &fractal::fractal::Fractal,
    f2: &fractal::fractal::Fractal
)
    -> fractal::fractal::Fractal
{
    f1.combine(f2).expect("failed combining")
}

pub async fn combine(mut conn: DbConn, Path((id1, id2)): Path<(i64, i64)>) -> Result<Json<SubmitDetails>, StatusCode> {
    blocking(move || {
    use crate::schema::fractals;

    // get the two fractals from database
    let json1 = fractals::table.select(fractals::json)
        .find(id1)
        .first::<String>(&mut *conn)
        .unwrap();
    let json2 = fractals::table.select(fractals::json)
        .find(id2)
        .first::<String>(&mut *conn)
        .unwrap();

    let f1 = json2fractal(&json1);
    let f2 = json2fractal(&json2);

    let f = combine_fractals(&f1, &f2);

    let (id, high, low) = add_fractal_to_db(&mut conn, &f.json());

    Json(
        SubmitDetails {
            id,
            low,
            high
        }
    )
    }).await
}

pub async fn random(mut conn: DbConn) -> Result<String, StatusCode> {
    blocking(move || {
    use crate::schema::fractals;
    use diesel::dsl::sql;

    let id = fractals::table.select(fractals::id)
        .filter(fractals::rank.gt(0))
        .limit(1)
        .order(sql::<Numeric>("RANDOM()"))
        .first::<i64>(&mut *conn)
        .expect("Error getting random fractals");

    format!("{}", id)
    }).await
}

pub async fn breed(State(tera): State<Arc<tera::Tera>>) -> Result<Html<String>, StatusCode> {
    let context: HashMap<&str, &str> = HashMap::new();

    render_template(&tera, "breed", &context)
}
