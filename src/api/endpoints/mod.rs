pub mod axis;
pub mod error;
pub mod gcode;
pub mod heating;

use crate::{ api::values::ApiError, hw::HwCtrl};
use rocket::{catch, data::FromData, post, response::status, serde::json::Json, Responder, State};

pub(self) type JsonResult<'r, T> = Result<Json<T>, <Json<T> as FromData<'r>>::Error>;

macro_rules! json_ok_or {
    ($json_res:ident, $err:expr) => {{
        match $json_res {
            Ok(json) => json.0,
            Err(_) => return $err,
        }
    }};
}

pub(self) use json_ok_or;

#[derive(Responder)]
pub enum ApiPutSettingsResponse {
    #[response(status = 200)]
    Ok(()),
    #[response(status = 405)]
    InvalidInput(()),
    #[response(status = 512)]
    SavingError(Json<ApiError>),
}

#[post("/estop")]
pub fn post_estop(hw_ctrl: &State<HwCtrl>) -> status::Accepted<()> {
    hw_ctrl.estop();
    status::Accepted(None)
}

#[catch(404)]
pub fn catch_404() -> () {
    ()
}
