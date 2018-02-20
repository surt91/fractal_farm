table! {
    fractals (id) {
        id -> BigInt,
        created_time -> BigInt,
        json -> Text,
        consumed -> Bool,
        deleted -> Bool,
        deleted_time -> Nullable<BigInt>,
        rank -> Nullable<BigInt>,
    }
}
