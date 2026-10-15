-- Root's watch history on top of seed.sql and movie.sql, newest first: the
-- seeded show's first episode rewatched, the movie a day before, the show's
-- three episodes back to back a day before that, and the movie thirty-five
-- times a week apart before all of it, so the history runs past its first page.

INSERT INTO watched_episodes (user_id, timestamp, show_id, season, episode)
VALUES
    ((SELECT id FROM users WHERE login = 'root'), 1760000000000, 1001, 1, 1),
    ((SELECT id FROM users WHERE login = 'root'), 1760002400000, 1001, 1, 2),
    ((SELECT id FROM users WHERE login = 'root'), 1760004800000, 1001, 1, 3),
    ((SELECT id FROM users WHERE login = 'root'), 1760172800000, 1001, 1, 1);

INSERT INTO watched_movies (user_id, timestamp, movie_id)
VALUES ((SELECT id FROM users WHERE login = 'root'), 1760086400000, 4001);

WITH RECURSIVE
    n (i) AS (
        SELECT 1
        UNION ALL
        SELECT i + 1 FROM n WHERE i < 35
    )
INSERT INTO watched_movies (user_id, timestamp, movie_id)
SELECT (SELECT id FROM users WHERE login = 'root'), 1760000000000 - i * 604800000, 4001 FROM n;
