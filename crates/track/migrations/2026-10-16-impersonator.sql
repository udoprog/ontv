-- Records the admin behind an impersonated session, as yew-auth's SqllStore
-- expects as of d463305 (docs/sqll/impersonator.sql). Existing sessions are
-- not impersonated.
ALTER TABLE sessions
ADD COLUMN impersonator_id INTEGER REFERENCES users (id) ON DELETE CASCADE;
