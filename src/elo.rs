fn expected(r_a: f64, r_b: f64) -> f64 {
    1. / (10f64.powf((r_a - r_b) / 400.) + 1.)
}

pub fn update(r_winner: i64, r_loser: i64) -> (i64, i64) {
    let k = 20.;
    let r_winner = r_winner as f64;
    let r_loser = r_loser as f64;

    let r_winner = r_winner + k * (1. - expected(r_winner, r_loser));
    let r_loser = r_loser + k * (0. - expected(r_loser, r_winner));

    let r_winner = r_winner.round() as i64;
    let r_loser = r_loser.round() as i64;

    (r_winner, r_loser)
}
