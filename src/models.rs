#[derive(Queryable, Serialize)]
pub struct Fractal {
    pub id: i64,
    pub created_time: i64,
    pub json: String,
    pub score: Option<i64>,
}
