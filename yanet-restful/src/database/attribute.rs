use super::DB;
use anyhow::Result;
use serde_json::Value;
use sqlx::query;

pub async fn upsert_attribute(peer_id: &str, attribute: &str, value: Value) -> Result<()> {
    query!(
        r#"
insert into attribute (attribute_peer_id, attribute_name, attribute_data)
values($1, $2, $3) 
on conflict (attribute_peer_id, attribute_name)
do update
set attribute_data = $3

        "#,
        peer_id,
        attribute,
        value
    )
    .execute(&*DB)
    .await?;
    Ok(())
}

pub async fn get_attribute(peer_id: &str, attribute: &str) -> Result<Value> {
    Ok(query!(
        r#"
select attribute_data from attribute 
where attribute_peer_id = $1
and attribute_name = $2
    "#,
        peer_id,
        attribute
    )
    .fetch_one(&*DB)
    .await?
    .attribute_data)
}
