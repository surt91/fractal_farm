FROM debian:bullseye-slim as builder
WORKDIR /usr/src/fractal
RUN apt-get update && apt-get install -y curl gcc libsqlite3-dev && rm -rf /var/lib/apt/lists/*
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup.sh
RUN sh rustup.sh -y --default-toolchain nightly --profile minimal
RUN $HOME/.cargo/bin/cargo install diesel_cli --no-default-features --features sqlite
COPY . .
RUN $HOME/.cargo/bin/cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libsqlite3-0 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/fractal/target/release/fractal_farm /usr/local/bin/fractal_farm
COPY --from=builder /root/.cargo/bin/diesel /usr/local/bin/diesel
COPY migrate_db.sh migrate_db.sh
COPY migrations/ migrations/
COPY static/ static/
COPY templates/ templates/
COPY Cargo.toml Cargo.toml
COPY Rocket.toml Rocket.toml
ENV DATABASE_URL=/db/db.sqlite
ENTRYPOINT ["/bin/bash", "-c", "./migrate_db.sh && fractal_farm"]