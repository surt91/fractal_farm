use super::diesel;
use diesel::prelude::*;

use super::db::DbConn;

use super::MAX;

// will move all entries between min and max by offset ranks
// you have to make sure that the unique constraint will not be violated
pub fn offset_rank(conn: &DbConn, min: i64, max: i64, offset: i64) {
    use schema::fractals::dsl::*;

    let safe = if MAX > MAX + offset { MAX } else { MAX + offset };

    diesel::update(
            fractals.filter(rank.ge(min))
                .filter(rank.le(max))
        )
        .set(rank.eq(rank + safe))
        .execute(&**conn)
        .expect("Error moving outside");

    diesel::update(
            fractals.filter(rank.ge(min+safe))
                .filter(rank.le(max+safe))
        )
        .set(rank.eq(rank - safe + offset))
        .execute(&**conn)
        .expect("Error moving by offset");
}
