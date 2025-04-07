CREATE TABLE user_file_uploads (
  id         UUID DEFAULT GEN_RANDOM_UUID() PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES users (id),
  name       TEXT NOT NULL,
  sha256sum  BYTEA NOT NULL,
  size       BIGINT,
  path       TEXT NOT NULL,
  mime_type  TEXT NOT NULL,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW()
);
