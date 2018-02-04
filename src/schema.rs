table! {
    fractals (id) {
        id -> BigInt,
        created_time -> BigInt,
        json -> Text,
        score -> Nullable<BigInt>,
        wins -> BigInt,
    }
}
