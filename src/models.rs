use super::schema::fractals;

#[derive(Queryable, Serialize, Debug, Clone)]
pub struct Fractal {
    pub id: i64,
    pub created_time: i64,
    pub json: String,
    pub consumed: bool,
    pub rank: Option<i64>
}

#[derive(Insertable, FromForm, Debug)]
#[table_name="fractals"]
pub struct NewFractal {
    pub json: String,
    pub rank: Option<i64>,
}
