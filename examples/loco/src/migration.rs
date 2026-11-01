use crate::models::{categories, products, sessions, users};
use sea_orm::{ConnectionTrait, Schema};
use sea_orm_migration::prelude::*;

pub struct Migrator;
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(Initial)]
    }
}
#[derive(DeriveMigrationName)]
struct Initial;
#[async_trait::async_trait]
impl MigrationTrait for Initial {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let schema = Schema::new(manager.get_connection().get_database_backend());
        for table in [
            schema.create_table_from_entity(users::Entity),
            schema.create_table_from_entity(sessions::Entity),
            schema.create_table_from_entity(categories::Entity),
            schema.create_table_from_entity(products::Entity),
        ] {
            manager.create_table(table).await?;
        }
        Ok(())
    }
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Custom(
            "This example does not provide destructive down migrations.".into(),
        ))
    }
}
