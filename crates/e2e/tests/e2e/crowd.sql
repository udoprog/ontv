-- Thirty more people than seed.sql, "Extra 01" to "Extra 30" in English
-- (6148257917992593152, as in seed.sql) and without credits, so the people
-- list runs past its first page.
WITH RECURSIVE
    n (i) AS (
        SELECT 1
        UNION ALL
        SELECT i + 1 FROM n WHERE i < 30
    )
INSERT INTO people (id, default_language)
SELECT 5200 + i, 6148257917992593152 FROM n;

INSERT INTO person_strings (person_id, language, kind, text)
SELECT id, 6148257917992593152, 1, printf('Extra %02d', id - 5200)
FROM people WHERE id > 5200;
