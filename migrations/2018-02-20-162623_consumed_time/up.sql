ALTER TABLE fractals ADD COLUMN consumed_time TEXT;
CREATE INDEX idx_consumed_time ON fractals (consumed, consumed_time);
