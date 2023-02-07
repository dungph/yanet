use serde::Deserialize;
use tide::{Request, Result};

use crate::{
    api::ApiResult,
    database::{self, account::valid_password},
};

pub async fn new_peer(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct AccountInfo {
        username: String,
        password: String,
        peer_id: String,
    }
    if let Ok(AccountInfo {
        username,
        password,
        peer_id,
    }) = req.body_json().await
    {
        match valid_password(&username, &password).await {
            Ok(true) => match database::peer::get_list_peer(&username).await {
                Ok(list) => ApiResult::success("Success", list).into(),
                Err(e) => ApiResult::failure(e, ()).into(),
            },
            Ok(false) => ApiResult::failure("Password not correct", ()).into(),
            Err(e) => ApiResult::failure(e, ()).into(),
        }
    } else {
        ApiResult::failure("Invalid Input", ()).into()
    }
}
pub async fn all_peer(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct AccountInfo {
        username: String,
        password: String,
    }
    if let Ok(AccountInfo { username, password }) = req.body_json().await {
        match valid_password(&username, &password).await {
            Ok(true) => match database::peer::get_list_peer(&username).await {
                Ok(list) => ApiResult::success("Success", list).into(),
                Err(e) => ApiResult::failure(e, ()).into(),
            },
            Ok(false) => ApiResult::failure("Password not correct", ()).into(),
            Err(e) => ApiResult::failure(e, ()).into(),
        }
    } else {
        ApiResult::failure("Invalid Input", ()).into()
    }
}
