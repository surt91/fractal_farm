use super::schema::fractals;

#[derive(Queryable, Serialize, Debug, Clone)]
pub struct Fractal {
    pub id: i64,
    pub created_time: i64,
    pub json: String,
    pub consumed: bool,
    pub consumed_time: Option<i64>,
    pub deleted: bool,
    pub deleted_time: Option<i64>,
    pub rank: Option<i64>
}

#[derive(Insertable, FromForm, Debug)]
#[diesel(table_name = fractals)]
pub struct NewFractal {
    pub json: String,
    pub rank: Option<i64>,
}
