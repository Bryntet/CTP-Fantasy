-- This file should undo anything in `up.sql`
ALTER TABLE tournaments 
DROP COLUMN running,
DROP COLUMN finished;

