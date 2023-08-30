//! `SeaORM` Entity. Generated by sea-orm-codegen 0.10.7
use anyhow::Result;
use ethereum_types::U256;
use ethportal_api::types::node_id::NodeId;
use ethportal_api::utils::bytes::hex_encode;

use sea_orm::{
    entity::prelude::*, ActiveValue::NotSet, DatabaseBackend, FromQueryResult, QueryOrder,
    QuerySelect, Set,
};
use sea_query::Expr;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "node")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub node_id: Vec<u8>,
    pub node_id_high: i64,
}

impl Model {
    pub fn node_id_as_hex(&self) -> String {
        hex_encode(&self.node_id)
    }

    pub fn get_node_id(&self) -> NodeId {
        NodeId(self.node_id.to_owned().try_into().expect("failed"))
    }
}

#[derive(FromQueryResult)]
pub struct ModelWithDistance {
    pub id: i32,
    pub node_id: Vec<u8>,
    pub node_id_high: i64,
    pub distance: i64,
}

impl ModelWithDistance {
    pub fn node_id_as_hex(&self) -> String {
        hex_encode(&self.node_id)
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::record::Entity")]
    Record,
    #[sea_orm(has_one = "super::client_info::Entity")]
    ClientInfo,
}

impl Related<super::record::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Record.def()
    }
}

impl Related<super::client_info::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ClientInfo.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Copy, Clone, Debug, EnumIter, DeriveColumn)]
enum ComputedColumn {
    Distance,
}

impl ColumnTrait for ComputedColumn {
    type EntityName = Entity;

    fn def(&self) -> ColumnDef {
        match self {
            Self::Distance => ColumnType::Integer.def(),
        }
    }
}

pub async fn closest_xor(
    node_id: NodeId,
    conn: &DatabaseConnection,
) -> Result<Vec<ModelWithDistance>> {
    let raw_node_id = U256::from_big_endian(&node_id.0);
    let node_id_high: i64 = (raw_node_id >> 193).as_u64().try_into().unwrap();

    let distance_expression = match conn.get_database_backend() {
        DatabaseBackend::Sqlite => Expr::cust_with_values(
            "(\"node\".\"node_id_high\" | ?1) - (\"node\".\"node_id_high\" & ?2)",
            [node_id_high, node_id_high],
        ),
        DatabaseBackend::Postgres => Expr::cust_with_values(
            "(\"node\".\"node_id_high\" | $1) - (\"node\".\"node_id_high\" & $2)",
            [node_id_high, node_id_high],
        ),
        DatabaseBackend::MySql => panic!("Unsupported"),
    };

    let nodes = Entity::find()
        .column_as(distance_expression, "distance")
        .order_by_asc(Expr::col(ComputedColumn::Distance))
        .limit(100)
        .into_model::<ModelWithDistance>()
        .all(conn)
        .await?;
    Ok(nodes)
}

pub async fn get_or_create(node_id: NodeId, conn: &DatabaseConnection) -> Result<Model> {
    // First try to lookup an existing entry.
    if let Some(node_id_model) = Entity::find()
        .filter(Column::NodeId.eq(node_id.0.to_vec()))
        .one(conn)
        .await?
    {
        // If there is an existing record, return it
        return Ok(node_id_model);
    }

    // If no record exists, create one and return it
    let raw_node_id = U256::from_big_endian(&node_id.0);
    let node_id_high: i64 = (raw_node_id >> 193).as_u64().try_into().unwrap();

    let node_id_model = ActiveModel {
        id: NotSet,
        node_id: Set(node_id.0.into()),
        node_id_high: Set(node_id_high),
    };

    Ok(node_id_model.insert(conn).await?)
}
