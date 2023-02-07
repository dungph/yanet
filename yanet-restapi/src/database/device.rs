use super::DB;
use anyhow::Result;
use serde_json::Value;
use sqlx::query;

pub async fn upsert_device(peer_id: &str, device_name: &str, value: Value) -> Result<()> {
    query!(
        r#"
insert into device (device_peer_id, device_name, device_data)
values($1, $2, $3)
on conflict (device_peer_id, device_name) do nothing
        "#,
        peer_id,
        device_name,
        value
    )
    .execute(&*DB)
    .await?;
    Ok(())
}

pub async fn get_device(peer_id: &str, device: &str) -> Result<Value> {
    Ok(query!(
        r#"
select device_data from device
where device_peer_id = $1
and device_name = $2
    "#,
        peer_id,
        device
    )
    .fetch_one(&*DB)
    .await?
    .device_data)
}

pub async fn get_list_device(peer_id: &str) -> Result<Vec<String>> {
    Ok(query!(
        r#"
select device_name from device 
join peer
on device_peer_id = peer_id
where peer_id = $1
        "#,
        peer_id
    )
    .fetch_all(&*DB)
    .await?
    .into_iter()
    .map(|o| o.device_name)
    .collect())
}
