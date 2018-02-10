use std::collections::HashMap;

use rocket::response::Redirect;
use rocket::request::Form;
use rocket_contrib::Template;

use super::diesel;
use diesel::prelude::*;

use super::db::DbConn;
use super::db_convenience;

use super::MAX;

#[get("/rate/<id>/<high>/<low>")]
pub fn rate(conn: DbConn, id: i64, high: i64, low: i64) -> Template {
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

#[derive(FromForm)]
pub struct DuelResult {
    candidate: i64,
    pivot: i64,
    low: i64,
    high: i64,
}

#[post("/below", data = "<result>")]
pub fn below(conn: DbConn, result: Form<DuelResult>) -> Redirect {
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

    if pivot_rank > candidate_rank {
        // set the candidate rank to NULL
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> null");

        db_convenience::offset_rank(&conn, pivot_rank, candidate_rank, -1);

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
pub fn above(conn: DbConn, result: Form<DuelResult>) -> Redirect {
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

    println!("above: pivot id: {}", pivot);
    println!("above: candidate id: {}", candidate);

    let pivot_rank = fractals::table.select(fractals::rank)
        .find(pivot)
        .first::<Option<i64>>(&*conn)
        .expect("abve: Can not find pivot")
        .unwrap();

    let candidate_rank = fractals::table.select(fractals::rank)
        .find(candidate)
        .first::<Option<i64>>(&*conn)
        .expect("abve: Can not find candidate")
        .unwrap_or(MAX + 1);

    println!("above: pivot: {}", pivot_rank);
    println!("above: candidate: {}", candidate_rank);

    if pivot_rank < candidate_rank {
        // set the candidate rank to NULL
        diesel::update(fractals::table.find(candidate))
            .set(fractals::rank.eq::<Option<i64>>(None))
            .execute(&*conn)
            .expect("Error saving new entry: winner -> null");

        db_convenience::offset_rank(&conn, pivot_rank, candidate_rank, 1);

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
