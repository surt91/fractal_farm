# Fractal Farm

This webserver renders fractal and saves details to reproduce them
together with a score.

This should gather data to improve fractals via some optimization method.


## Todo

*   [x] submit a score (and the json) to be saved
*   [x] save json details in database (sqlite)
*   [ ] make an endpoint to submit json
*   [ ] json could be directly editable
*   [ ] option to download a high resolution version
*   [ ] better html/css suited for mobile


## Setup the Database

```bash
export DATABASE_URL=sqlite:///foo.db
cargo install diesel_cli
diesel setup
diesel migration run
```
