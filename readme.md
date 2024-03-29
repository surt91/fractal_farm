# Fractal Farm

This webserver renders fractal and saves details to reproduce them
together with a score.

This should gather data to improve fractals via some optimization method.


## Todo

*   [x] submit a score (and the json) to be saved
*   [x] save json details in database (sqlite)
*   [x] make an endpoint to submit json
*   [x] json could be directly editable
*   [x] option to download a high resolution version
*   [ ] better html/css suited for mobile
*   [x] for rating render in draft mode
*   [x] create indices on consumed, rank, created_time
*   [x] use bisection
*   [x] genetic combination of transformations
*   [ ] genetic mutation of transformations

## Setup the Database

```bash
export DATABASE_URL=db.sqlite
cargo install diesel_cli
diesel setup
diesel migration run
```

## Using Docker

Build it and run it with

```bash
docker build . -t farm
mkdir -p db  # create dir for the database
docker run -v $PWD/db:/db -p 7878:7878 farm
```

Or using `docker-compose`

```bash
docker-compose up
```