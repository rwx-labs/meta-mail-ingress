CREATE TABLE users (
  id           SERIAL PRIMARY KEY,
  email        TEXT UNIQUE NOT NULL,
  access_token TEXT NOT NULL,
  enabled      BOOLEAN DEFAULT TRUE
);
