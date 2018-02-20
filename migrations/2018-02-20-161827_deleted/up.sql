ALTER TABLE fractals ADD COLUMN deleted BOOLEAN NOT NULL DEFAULT 0;
UPDATE fractals SET deleted=0;
ALTER TABLE fractals ADD COLUMN deleted_time TEXT;
CREATE INDEX idx_deleted ON fractals (deleted);
CREATE INDEX idx_deleted_time ON fractals (deleted, deleted_time);
