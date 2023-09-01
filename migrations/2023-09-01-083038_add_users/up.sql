-- Your SQL goes here
CREATE TABLE public.users (
    id SERIAL PRIMARY KEY,
    username TEXT UNIQUE NOT NULL
);

CREATE TABLE public.user_selections (
    id SERIAL PRIMARY KEY,
    user_id INTEGER REFERENCES users(id) NOT NULL,
    player_id INTEGER REFERENCES players(pdga_number) NOT NULL,
    UNIQUE (user_id, player_id)
);
