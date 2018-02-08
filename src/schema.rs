table! {
    fractals (id) {
        id -> BigInt,
        created_time -> BigInt,
        json -> Text,
        consumed -> Bool,
        rank -> Nullable<BigInt>,
    }
}
