use serde::Deserialize;
use tide::Request;
use tide::Result;

use crate::api::ApiResult;
use crate::database;
use crate::database::account::valid_password;

pub async fn all_device(mut req: Request<()>) -> Result {
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
            Ok(true) => match database::device::get_list_device(&peer_id).await {
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

pub async fn device_data(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct AccountInfo {
        username: String,
        password: String,
        peer_id: String,
        device_name: String,
    }
    if let Ok(AccountInfo {
        username,
        password,
        peer_id,
        device_name,
    }) = req.body_json().await
    {
        match valid_password(&username, &password).await {
            Ok(true) => match database::device::get_device(&peer_id, &device_name).await {
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
