use diesel::prelude::*;
use crate::*;


pub trait SQLStuff {
    fn exists(&self, conn: &mut PgConnection) -> bool;
    fn insert_into_sql(&self, conn: &mut PgConnection);
    fn from_id(id: i32, conn: &mut PgConnection) -> Option<Self> where Self: Sized;
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = schema::tournaments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Debug)]
#[derive(Insertable)]
pub struct Tournament {
    pub tournament_id: i32,
    pub running: bool,
    pub finished: bool,
}

impl SQLStuff for Tournament {
    fn insert_into_sql(&self, conn: &mut PgConnection) {
        use schema::tournaments::dsl::*;
        diesel::insert_into(tournaments)
            .values(self)
            .execute(conn)
            .expect("Error saving new post");
    }

    fn exists(&self, conn: &mut PgConnection) -> bool {
        use schema::tournaments::dsl::*;
        diesel::select(diesel::dsl::exists(
            tournaments.filter(tournament_id.eq(self.tournament_id))
        ))
        .get_result(conn)
        .expect("Error checking if tournament exists")
    }

    fn from_id(id: i32, conn: &mut PgConnection) -> Option<Tournament> {
        use schema::tournaments::dsl::*;
        tournaments
            .filter(tournament_id.eq(id))
            .first::<Tournament>(conn)
            .optional()
            .expect("Error loading tournament")
    }
}

#[derive(Queryable, Selectable)]
#[diesel(table_name = schema::players)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Debug)]
#[derive(Insertable)]
pub struct Player {
    pub pdga_number: i32,
    pub first_name: String,
    pub last_name: Option<String>,
    pub rating: Option<i32>,
    pub avatar: Option<String>,
}

impl SQLStuff for Player {
    fn insert_into_sql(&self, conn: &mut PgConnection) {
        if self.pdga_number == 0 {
            return;
        }
        use schema::players::dsl::*;
        diesel::insert_into(players)
            .values(self)
            .execute(conn)
            .expect("Error saving new post");
    }

    fn exists(&self, conn: &mut PgConnection) -> bool {
        use schema::players::dsl::*;
        diesel::select(diesel::dsl::exists(
            players.filter(pdga_number.eq(self.pdga_number))
        ))
        .get_result(conn)
        .expect("Error checking if player exists")
    }

    fn from_id(id: i32, conn: &mut PgConnection) -> Option<Player> {
        use schema::players::dsl::*;
        players
            .filter(pdga_number.eq(id))
            .first::<Player>(conn)
            .optional()
            .expect("Error loading player")
    }
}

impl Player {
    pub fn get_running_tournament(&self, conn: &mut PgConnection) -> Option<Tournament> {
        use schema::player_tournaments;
        let ids: Vec<i32> = player_tournaments::table
            .filter(player_tournaments::player_id.eq(self.pdga_number))
            .select(player_tournaments::tournament_id)
            .load(conn)
            .expect("Error loading tournaments");

        ids.iter().filter_map(|id| {Tournament::from_id(*id, conn)}).find(|t| t.running)
    }
}


#[derive(Queryable, Selectable)]
#[diesel(table_name = schema::users)]
#[diesel(check_for_backend(diesel::pg::Pg))]
#[derive(Debug)]
#[derive(Insertable)]
pub struct User {
    pub id: i32,
    pub username: String,
}

impl SQLStuff for User {
    fn insert_into_sql(&self, conn: &mut PgConnection) {
        use schema::users::dsl::*;
        diesel::insert_into(users)
            .values(self)
            .execute(conn)
            .expect("Error saving new user");
    }

    fn exists(&self, conn: &mut PgConnection) -> bool {
        use schema::users::dsl::*;
        diesel::select(diesel::dsl::exists(
            users.filter(id.eq(self.id))
        ))
        .get_result(conn)
        .expect("Error checking if user exists")
    }

    fn from_id(id: i32, conn: &mut PgConnection) -> Option<User> {
        use schema::users;
        users::dsl::users
            .filter(users::id.eq(id))
            .first::<User>(conn)
            .optional()
            .expect("Error loading user")
    }
}

impl User {
    pub fn get_players(&self, conn: &mut PgConnection) -> Vec<Player> {
        use schema::{ players, users, user_selections };

        user_selections::table
            .inner_join(players::table.on(user_selections::player_id.eq(players::pdga_number))) // link players to user_selections id 
            .inner_join(users::table.on(user_selections::user_id.eq(users::id)))
            .filter(users::id.eq(user_selections::user_id))
            .select(players::table::all_columns()) 
            .load::<Player>(conn)
            .expect("Error loading players")
    }
}

fn get_players_without_avatar(conn: &mut PgConnection) -> Vec<i32> {
    use schema::players::dsl::*;
    players
        .filter(avatar.is_null())
        .select(pdga_number)
        .load(conn)
        .expect("Error loading posts")
}


