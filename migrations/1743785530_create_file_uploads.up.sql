CREATE TABLE file_uploads (
  id         UUID DEFAULT GEN_RANDOM_UUID() PRIMARY KEY,
  user_id    INTEGER NOT NULL REFERENCES users (id),
  name       TEXT NOT NULL,
  sha256sum  BYTEA NOT NULL,
  size       BIGINT,
  created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
  path       TEXT NOT NULL
);
