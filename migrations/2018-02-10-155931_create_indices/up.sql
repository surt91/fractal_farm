CREATE UNIQUE INDEX idx_rank ON fractals (rank);
CREATE INDEX idx_created_time ON fractals (created_time);
CREATE INDEX idx_consumed ON fractals (consumed);
CREATE INDEX idx_consumed_rank ON fractals (consumed, rank);
