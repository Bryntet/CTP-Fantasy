use sea_orm::DatabaseConnection;

use crate::objects::FantasyPick;

impl FantasyPick {
    async fn insert_or_change(&self, db: &DatabaseConnection, user_id: i32) -> Result<(), GenericError> {
        use entity::prelude::FantasyPick as FantasyPickEntity;
        use sea_orm::{ColumnTrait, NotSet, QueryFilter, Set};

        let existing_pick = FantasyPickEntity::find()
            .filter(entity::fantasy_pick::Column::PickNumber.eq(self.slot))
            .filter(entity::fantasy_pick::Column::User.eq(user_id))
            .filter(
                entity::fantasy_pick::Column::FantasyTournamentId.eq(self.fantasy_tournament_id),
            )
            .one(db)
            .await?;

        if !service::player_exists(db, self.pdga_number).await {
            Err::<(), GenericError>(error::PlayerError::PlayerNotFound("Unknown player id").into())?;
        }
        match existing_pick {
            Some(pick) => {
                let mut pick: entity::fantasy_pick::ActiveModel = pick.into();
                pick.player = Set(self.pdga_number);
                pick.update(db).await?;
            }
            None => {
                let new_pick = entity::fantasy_pick::ActiveModel {
                    id: NotSet,
                    user: Set(user_id),
                    pick_number: Set(self.slot),
                    player: Set(self.pdga_number),
                    fantasy_tournament_id: Set(self.fantasy_tournament_id),
                    division: Set(service::get_player_division(db, self.pdga_number)
                        .await?
                        .first()
                        .unwrap()
                        .to_owned()),
                };
                new_pick.insert(db).await?;
            }
        }
        Ok(())
    }
}