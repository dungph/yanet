use crate::{
    api::ApiResult,
    database::{self, account, attribute},
};
use async_channel::{unbounded, Receiver, Sender};
use base58::FromBase58;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde_json::Value;
use tide::{Request, Result};
use yanet_core::PeerId;

type Pair<T> = (Sender<T>, Receiver<T>);
static REQ: Lazy<Pair<(PeerId, String, Value)>> = Lazy::new(|| unbounded());

pub async fn wait_req() -> (PeerId, String, Value) {
    REQ.1.recv().await.unwrap()
}

pub async fn set_attribute(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct Input {
        username: String,
        password: String,
        peer_id: String,
        key: String,
        value: Value,
    }
    if let Ok(Input {
        username,
        password,
        peer_id,
        key,
        value,
    }) = req.body_json().await
    {
        match account::valid_password(&username, &password).await {
            Ok(true) => {
                println!("1\n\n\n\n\n");
                if let Ok(true) = attribute::is_owned_by_account(&username, &peer_id, &key).await {
                    println!("2\n\n\n\n\n");
                    if let Ok(pubkey) = peer_id.from_base58() {
                        println!("3\n\n\n\n\n");
                        if let Ok(peer_id) = PeerId::try_from(pubkey.as_slice()) {
                            println!("4\n\n\n\n\n");
                            REQ.0.send((peer_id, key, value.into())).await?;
                            return ApiResult::success("Success", ()).into();
                        }
                    }
                }
                ApiResult::failure("Invalid input", ()).into()
            }
            Ok(false) => ApiResult::failure("Wrong account or password", ()).into(),
            Err(e) => ApiResult::failure(&format!("{e}"), ()).into(),
        }
    } else {
        ApiResult::failure("Invalid input", ()).into()
    }
}

pub async fn get_attribute(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct Input {
        username: String,
        password: String,
        peer_id: String,
        key: String,
    }
    if let Ok(Input {
        username,
        password,
        peer_id,
        key,
    }) = req.body_json().await
    {
        match account::valid_password(&username, &password).await {
            Ok(true) => {
                if let Ok(true) = attribute::is_owned_by_account(&username, &peer_id, &key).await {
                    let value = database::attribute::get_attribute(&peer_id, &key).await?;
                    return ApiResult::success("Success", value).into();
                }
                ApiResult::failure("Invalid input", ()).into()
            }
            Ok(false) => ApiResult::failure("Wrong account or password", ()).into(),
            Err(e) => ApiResult::failure(&format!("{e}"), ()).into(),
        }
    } else {
        ApiResult::failure("Invalid input", ()).into()
    }
}
