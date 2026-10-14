-- Strings of unknown language are stored under `und` (1970168832, see
-- api::Locale::UNKNOWN). Kinds: 3 character, 4 job, 5 department.
--
-- A credit's character names synced under every locale but identical in all of
-- them were never translations: they collapse into one row of unknown language.
-- Credits whose names differ between locales keep them as they are.
INSERT INTO
    show_credit_strings (credit_id, language, kind, text)
SELECT
    credit_id,
    1970168832,
    kind,
    MIN(text)
FROM
    show_credit_strings
GROUP BY
    credit_id,
    kind
HAVING
    COUNT(DISTINCT text) = 1;

DELETE FROM show_credit_strings
WHERE
    language != 1970168832
    AND (credit_id, kind) IN (
        SELECT
            credit_id,
            kind
        FROM
            show_credit_strings
        WHERE
            language = 1970168832
    );

INSERT INTO
    movie_credit_strings (credit_id, language, kind, text)
SELECT
    credit_id,
    1970168832,
    kind,
    MIN(text)
FROM
    movie_credit_strings
GROUP BY
    credit_id,
    kind
HAVING
    COUNT(DISTINCT text) = 1;

DELETE FROM movie_credit_strings
WHERE
    language != 1970168832
    AND (credit_id, kind) IN (
        SELECT
            credit_id,
            kind
        FROM
            movie_credit_strings
        WHERE
            language = 1970168832
    );

-- Person names (kind 1) the same way: the remote does not translate them.
INSERT INTO
    person_strings (person_id, language, kind, text)
SELECT
    person_id,
    1970168832,
    kind,
    MIN(text)
FROM
    person_strings
WHERE
    kind = 1
GROUP BY
    person_id,
    kind
HAVING
    COUNT(DISTINCT text) = 1;

DELETE FROM person_strings
WHERE
    kind = 1
    AND language != 1970168832
    AND person_id IN (
        SELECT
            person_id
        FROM
            person_strings
        WHERE
            kind = 1
            AND language = 1970168832
    );

-- Jobs and departments were plain, untranslated columns.
INSERT INTO
    show_credit_strings (credit_id, language, kind, text)
SELECT
    id,
    1970168832,
    4,
    job
FROM
    show_credits
WHERE
    trim(coalesce(job, '')) != '';

INSERT INTO
    show_credit_strings (credit_id, language, kind, text)
SELECT
    id,
    1970168832,
    5,
    department
FROM
    show_credits
WHERE
    trim(coalesce(department, '')) != '';

INSERT INTO
    movie_credit_strings (credit_id, language, kind, text)
SELECT
    id,
    1970168832,
    4,
    job
FROM
    movie_credits
WHERE
    trim(coalesce(job, '')) != '';

INSERT INTO
    movie_credit_strings (credit_id, language, kind, text)
SELECT
    id,
    1970168832,
    5,
    department
FROM
    movie_credits
WHERE
    trim(coalesce(department, '')) != '';

INSERT INTO
    person_strings (person_id, language, kind, text)
SELECT
    id,
    1970168832,
    5,
    department
FROM
    people
WHERE
    trim(coalesce(department, '')) != '';

ALTER TABLE show_credits
DROP COLUMN department;

ALTER TABLE show_credits
DROP COLUMN job;

ALTER TABLE movie_credits
DROP COLUMN department;

ALTER TABLE movie_credits
DROP COLUMN job;

ALTER TABLE people
DROP COLUMN department;
