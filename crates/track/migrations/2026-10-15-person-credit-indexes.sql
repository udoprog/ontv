-- The people list sorts by and counts each person's credits.
CREATE INDEX idx_show_credits_person ON show_credits (person_id);

CREATE INDEX idx_movie_credits_person ON movie_credits (person_id);
