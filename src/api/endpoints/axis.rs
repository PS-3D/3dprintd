use super::{ApiPutSettingsResponse, JsonResult};
use crate::{api::values::Errors, comms::Axis, decode::DecoderCtrl, settings::Settings};
use rocket::{get, http::Status, post, put, response::status, serde::json::Json, State};
use serde::{Deserialize, Serialize};

#[get("/axis/position")]
pub fn get_position() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/axis/<axis_name>/position")]
pub fn get_axis_name_position(axis_name: Axis) -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[derive(Debug, Serialize)]
pub struct ApiGetAxisSettings {
    reference_speed: u32,
    reference_accel_decel: u32,
    reference_jerk: u32,
}

#[get("/axis/<axis_name>/settings")]
pub fn get_axis_name_settings(
    axis_name: Axis,
    settings: &State<Settings>,
) -> status::Custom<Json<ApiGetAxisSettings>> {
    // FIXME get all while locking once
    macro_rules! axis_setting {
        ($axis:ident, $setting_func:ident) => {
            settings.motors().$axis().$setting_func()
        };
    }
    macro_rules! make_settings {
        ($axis:ident) => {
            ApiGetAxisSettings {
                reference_speed: axis_setting!($axis, get_reference_speed),
                reference_accel_decel: axis_setting!($axis, get_reference_accel_decel),
                reference_jerk: axis_setting!($axis, get_reference_jerk),
            }
        };
    }
    let api_settings = match axis_name {
        Axis::X => make_settings!(x),
        Axis::Y => make_settings!(y),
        Axis::Z => make_settings!(z),
    };
    status::Custom(Status::Ok, Json(api_settings))
}

#[derive(Debug, Serialize)]
pub struct ApiGetExtruderSettings {}

#[get("/axis/e/settings")]
pub fn get_e_settings() -> status::Custom<Json<ApiGetExtruderSettings>> {
    status::Custom(Status::Ok, Json(ApiGetExtruderSettings {}))
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiPutAxisNameSettings {
    reference_speed: Option<u32>,
    reference_accel_decel: Option<u32>,
    reference_jerk: Option<u32>,
}

#[put("/axis/<axis_name>/settings", data = "<received_settings>")]
pub fn put_axis_name_settings(
    axis_name: Axis,
    received_settings: JsonResult<ApiPutAxisNameSettings>,
    settings: &State<Settings>,
    errors: &State<Errors>,
) -> ApiPutSettingsResponse {
    let received_settings = match received_settings {
        Ok(s) => s,
        Err(_) => return ApiPutSettingsResponse::InvalidInput(()),
    };
    // FIXME set all while locking once
    macro_rules! set_value {
        ($axis:ident, $set_func:ident, $field:ident) => {{
            if let Some(value) = received_settings.$field {
                settings.motors().$axis().$set_func(value);
            }
        }};
    }
    macro_rules! set_axis {
        ($axis:ident) => {{
            set_value!($axis, set_reference_speed, reference_speed);
            set_value!($axis, set_reference_accel_decel, reference_accel_decel);
            set_value!($axis, set_reference_jerk, reference_jerk);
        }};
    }
    match axis_name {
        Axis::X => set_axis!(x),
        Axis::Y => set_axis!(y),
        Axis::Z => set_axis!(z),
    }
    if let Err(e) = settings.save() {
        ApiPutSettingsResponse::SavingError(Json(errors.insert_get(e)))
    } else {
        ApiPutSettingsResponse::Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiPutExtruderSettings {}

#[put("/axis/e/settings", data = "<received_settings>")]
pub fn put_e_settings(
    received_settings: JsonResult<ApiPutExtruderSettings>,
) -> ApiPutSettingsResponse {
    let _received_settings = match received_settings {
        Ok(s) => s,
        Err(_) => return ApiPutSettingsResponse::InvalidInput(()),
    };
    ApiPutSettingsResponse::Ok(())
}

#[post("/axis/<axis_name>/reference")]
pub fn post_axis_name_reference(
    axis_name: Axis,
    decoder_ctrl: &State<DecoderCtrl>,
) -> Result<status::Accepted<()>, status::Custom<()>> {
    decoder_ctrl
        .try_reference_axis(axis_name)
        .map(|_| status::Accepted(None))
        .map_err(|_| status::Custom(Status { code: 409 }, ()))
}
