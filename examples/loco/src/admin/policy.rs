use reefadmin::{AccessEntity, AccessPolicy, AdminActor, Operation, Row};
use sea_orm::sea_query::{Condition, Expr, ExprTrait};

pub struct InventoryPolicy;
impl AccessPolicy for InventoryPolicy {
    fn can_enter(&self, actor: &AdminActor) -> bool {
        actor.claims["staff"] == true
    }
    fn scope(
        &self,
        actor: &AdminActor,
        entity: AccessEntity<'_>,
        operation: &Operation,
    ) -> Option<Condition> {
        let owner = actor.id.parse::<i32>().ok()?;
        let allowed = match (entity.name, operation) {
            ("products", Operation::View | Operation::Update | Operation::Export) => true,
            ("products", Operation::Toggle(field)) => field == "active",
            ("products", Operation::Action(action)) => action == "archive",
            ("categories", Operation::View | Operation::Autocomplete) => true,
            _ => false,
        };
        (self.can_enter(actor) && allowed).then(|| {
            Condition::all().add(Expr::col((entity.table.to_owned(), "owner_id")).eq(owner))
        })
    }
    fn can_write(
        &self,
        actor: &AdminActor,
        _entity: AccessEntity<'_>,
        operation: &Operation,
        current: Option<&Row>,
        proposed: &Row,
    ) -> bool {
        self.can_enter(actor)
            && matches!(operation, Operation::Update | Operation::Toggle(_))
            && current.is_some_and(|row| {
                row["owner_id"].as_i64().map(|id| id.to_string()).as_deref()
                    == Some(actor.id.as_str())
                    && proposed["owner_id"] == row["owner_id"]
            })
    }
}
