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

pub async fn is_owned_by_account(username: &str, peer_id: &str, attribute: &str) -> Result<bool> {
    Ok(query!(
        r#"
select attribute_name from attribute
join link_account_peer
on link_peer_id = attribute_peer_id
where link_account_username = $1
and link_peer_id = $2
and attribute_name = $3
            "#,
        username,
        peer_id,
        attribute
    )
    .fetch_optional(&*DB)
    .await?
    .is_some())
}
