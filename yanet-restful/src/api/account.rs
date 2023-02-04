use serde::Deserialize;
use tide::Request;
use tide::Result;

use crate::{api::ApiResult, database::account};

pub async fn create_account(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct AccountInfo {
        username: String,
        password: String,
    }

    if let Ok(AccountInfo { username, password }) = req.body_json().await {
        if let Ok(_) = account::create_account(&username, &password).await {
            ApiResult::success("Succes", ()).into()
        } else {
            ApiResult::failure("Failed to create account", ()).into()
        }
    } else {
        ApiResult::failure("Invalid Input", ()).into()
    }
}

pub async fn valid_password(mut req: Request<()>) -> Result {
    #[derive(Deserialize, Debug)]
    struct Login {
        username: String,
        password: String,
    }
    if let Ok(Login { username, password }) = dbg!(req.body_json().await) {
        match account::valid_password(&username, &password).await {
            Ok(val) => ApiResult::success("Success", val).into(),
            Err(e) => ApiResult::failure(&format!("{e}"), ()).into(),
        }
    } else {
        ApiResult::failure("Invalid input", ()).into()
    }
}

pub async fn new_password(mut req: Request<()>) -> Result {
    #[derive(Deserialize)]
    struct Input {
        username: String,
        password: String,
        new_password: String,
    }
    if let Ok(Input {
        username,
        password,
        new_password,
    }) = req.body_json().await
    {
        match account::valid_password(&username, &password).await {
            Ok(true) => match account::change_password(&username, &new_password).await {
                Ok(()) => ApiResult::success("Success", ()).into(),
                Err(e) => ApiResult::failure(e, ()).into(),
            },
            Ok(false) => ApiResult::failure("Wrong account or password", ()).into(),
            Err(e) => ApiResult::failure(&format!("{e}"), ()).into(),
        }
    } else {
        ApiResult::failure("Invalid input", ()).into()
    }
}
