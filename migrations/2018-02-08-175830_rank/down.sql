DROP TABLE fractals;
CREATE TABLE fractals (
  id INTEGER PRIMARY KEY NOT NULL,
  created_time INTEGER NOT NULL DEFAULT CURRENT_TIMESTAMP,
  json TEXT NOT NULL,
  score INTEGER,
  wins INTEGER NOT NULL DEFAULT 0,
  trials INTEGER NOT NULL DEFAULT 0,
  elo INTEGER NOT NULL DEFAULT 1000,
  consumed BOOLEAN NOT NULL DEFAULT 0
)
