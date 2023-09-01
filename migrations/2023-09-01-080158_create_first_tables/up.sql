-- Your SQL goes here


CREATE TABLE public.tournaments (
    tournament_id int4 NOT NULL PRIMARY KEY
);

CREATE TABLE public.players (
    pdga_number int4 NOT NULL PRIMARY KEY,
    first_name text NOT NULL,
    last_name text,
    rating int4,
    avatar text,
);

CREATE TABLE public.player_tournaments (
    id SERIAL PRIMARY KEY,
    player_id INT4 NOT NULL,
    tournament_id INT4 NOT NULL,
    UNIQUE (player_id, tournament_id),
    FOREIGN KEY (player_id) REFERENCES players(pdga_number),
    FOREIGN KEY (tournament_id) REFERENCES tournaments(tournament_id)
);

