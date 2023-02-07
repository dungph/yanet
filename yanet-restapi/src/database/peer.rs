use super::DB;
use anyhow::Result;
use sqlx::query;

pub async fn upsert_peer(peer_id: &str) -> Result<()> {
    query!(
        r#"
insert into  peer (peer_id ,peer_password)
values ($1, '')
on conflict (peer_id)
do nothing;
            "#,
        peer_id,
    )
    .execute(&*DB)
    .await?;
    query!(
        r#"
insert into link_account_peer (link_account_username, link_peer_id)
values ('admin', $1)
on conflict
do nothing;
        "#,
        peer_id
    )
    .execute(&*DB)
    .await?;
    Ok(())
}
pub async fn set_peer_password(peer_id: &str, peer_password: &str) -> Result<()> {
    query!(
        r#"
insert into  peer (peer_id ,peer_password)
values ($1, $2)
on conflict (peer_id)
do update set peer_password = $2;
            "#,
        peer_id,
        peer_password
    )
    .execute(&*DB)
    .await?;
    Ok(())
}

pub async fn get_peer_password(peer_id: &str) -> Result<Option<String>> {
    Ok(query!(
        r#"
select peer_password from peer
where peer_id = $1
        "#,
        peer_id
    )
    .fetch_optional(&*DB)
    .await?
    .map(|o| o.peer_password))
}

pub async fn get_list_peer(username: &str) -> Result<Vec<String>> {
    Ok(query!(
        r#"
select link_peer_id from link_account_peer
where link_account_username = $1
        "#,
        username
    )
    .fetch_all(&*DB)
    .await?
    .into_iter()
    .map(|o| o.link_peer_id)
    .collect())
}
