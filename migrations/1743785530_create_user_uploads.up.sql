CREATE TABLE user_uploads (
  id         UUID DEFAULT GEN_RANDOM_UUID() PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES users (id),
  sha256sum  BYTEA NOT NULL,
  mime_type  TEXT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);

CREATE INDEX user_uploads_sha256sums_idx ON user_uploads (sha256sum);
