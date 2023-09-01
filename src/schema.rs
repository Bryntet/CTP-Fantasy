// @generated automatically by Diesel CLI.

diesel::table! {
    player_tournaments (id) {
        id -> Int4,
        player_id -> Int4,
        tournament_id -> Int4,
    }
}

diesel::table! {
    players (pdga_number) {
        pdga_number -> Int4,
        first_name -> Text,
        last_name -> Nullable<Text>,
        rating -> Nullable<Int4>,
        avatar -> Nullable<Text>,
    }
}

diesel::table! {
    tournaments (tournament_id) {
        tournament_id -> Int4,
    }
}

diesel::table! {
    user_selections (id) {
        id -> Int4,
        user_id -> Int4,
        player_id -> Int4,
    }
}

diesel::table! {
    users (id) {
        id -> Int4,
        username -> Text,
    }
}

diesel::joinable!(player_tournaments -> players (player_id));
diesel::joinable!(player_tournaments -> tournaments (tournament_id));
diesel::joinable!(user_selections -> players (player_id));
diesel::joinable!(user_selections -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    player_tournaments,
    players,
    tournaments,
    user_selections,
    users,
);
