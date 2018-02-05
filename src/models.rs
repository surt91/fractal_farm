use super::schema::fractals;

#[derive(Queryable, Serialize, Debug, Clone)]
pub struct Fractal {
    pub id: i64,
    pub created_time: i64,
    pub json: String,
    pub score: Option<i64>,
    pub wins: i64,
    pub trials: i64,
    pub elo: i64,
    pub consumed: bool,
}

#[derive(Insertable, FromForm, Debug)]
#[table_name="fractals"]
pub struct NewFractal {
    pub json: String,
    pub score: Option<i64>,
}
