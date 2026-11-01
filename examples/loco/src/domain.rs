use crate::models::{products, users};
use reefadmin::{AdminActor, AdminError};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, IntoActiveModel, QueryFilter,
    QuerySelect, Set, TransactionTrait,
};

/// Host-owned operation. Recheck current access and roll back the entire selection on failure.
pub async fn archive(
    db: &DatabaseConnection,
    actor: &AdminActor,
    ids: &[i32],
) -> Result<(), AdminError> {
    let owner = actor
        .id
        .parse::<i32>()
        .map_err(|_| AdminError::AccessDenied)?;
    let tx = db.begin().await?;
    let user = users::Entity::find_by_id(owner)
        .lock_exclusive()
        .one(&tx)
        .await?
        .ok_or(AdminError::AccessDenied)?;
    if !user.is_staff {
        return Err(AdminError::AccessDenied);
    }
    for id in ids {
        let row = products::Entity::find_by_id(*id)
            .filter(products::Column::OwnerId.eq(owner))
            .lock_exclusive()
            .one(&tx)
            .await?
            .ok_or(AdminError::AccessDenied)?;
        if !row.active {
            return Err(AdminError::custom("The product is already archived."));
        }
        let mut row = row.into_active_model();
        row.active = Set(false);
        row.update(&tx).await?;
    }
    tx.commit().await?;
    Ok(())
}
