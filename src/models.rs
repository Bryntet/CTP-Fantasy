use diesel::prelude::*;

#[derive(Queryable, Selectable)]
#[diesel(table_name = crate::schema::players)]
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

impl Player {
    pub fn insert_into_sql(&self, conn: &mut PgConnection) {
        use crate::schema::players::dsl::*;
        diesel::insert_into(players)
            .values(self)
            .execute(conn)
            .expect("Error saving new post");
    }

    pub fn exists(&self, conn: &mut PgConnection) -> bool {
        use crate::schema::players::dsl::*;
        diesel::select(diesel::dsl::exists(
            players.filter(pdga_number.eq(self.pdga_number))
        ))
        .get_result(conn)
        .expect("Error checking if player exists")
    }
}