use super::{json_ok_or, ApiPutSettingsResponse, JsonResult};
use crate::{
    api::values::Errors,
    comms::{Axis, OnewayAtomicF64Read, OnewayDataRead},
    decode::DecoderCtrl,
    settings::Settings,
};
use rocket::{
    get, http::Status, post, put, request::FromParam, response::status, serde::json::Json,
    Responder, State,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiGetAxisNamePosition {
    position: f64,
}

impl From<&OnewayAtomicF64Read> for ApiGetAxisNamePosition {
    fn from(read: &OnewayAtomicF64Read) -> Self {
        Self {
            position: read.read(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiGetAxisPosition {
    x: ApiGetAxisNamePosition,
    y: ApiGetAxisNamePosition,
    z: ApiGetAxisNamePosition,
}

impl From<&OnewayDataRead> for ApiGetAxisPosition {
    fn from(read: &OnewayDataRead) -> Self {
        Self {
            x: (&read.pos_x).into(),
            y: (&read.pos_y).into(),
            z: (&read.pos_z).into(),
        }
    }
}

#[get("/axis/position")]
pub fn get_position(
    oneway_data_read: &State<OnewayDataRead>,
) -> status::Custom<Json<ApiGetAxisPosition>> {
    status::Custom(Status::Ok, Json(State::inner(oneway_data_read).into()))
}

#[get("/axis/<axis_name>/position")]
pub fn get_axis_name_position(
    axis_name: Axis,
    oneway_data_read: &State<OnewayDataRead>,
) -> status::Custom<Json<ApiGetAxisNamePosition>> {
    macro_rules! get_pos {
        ($axis:ident) => {
            (&oneway_data_read.$axis).into()
        };
    }
    let position = match axis_name {
        Axis::X => get_pos!(pos_x),
        Axis::Y => get_pos!(pos_y),
        Axis::Z => get_pos!(pos_z),
    };
    status::Custom(Status::Ok, Json(position))
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

#[derive(Debug, PartialEq, Eq)]
pub enum ApiPostAxisXYReferenceAxis {
    X,
    Y,
}

impl FromParam<'_> for ApiPostAxisXYReferenceAxis {
    type Error = &'static str;

    fn from_param(param: &str) -> Result<Self, Self::Error> {
        match param {
            "x" => Ok(Self::X),
            "y" => Ok(Self::Y),
            _ => Err("not x or y"),
        }
    }
}

impl From<ApiPostAxisXYReferenceAxis> for Axis {
    fn from(axis: ApiPostAxisXYReferenceAxis) -> Self {
        match axis {
            ApiPostAxisXYReferenceAxis::X => Self::X,
            ApiPostAxisXYReferenceAxis::Y => Self::Y,
        }
    }
}

#[derive(Debug, Responder)]
pub enum ApiPostAxisReferenceResponse {
    #[response(status = 202)]
    Accepted(()),
    #[response(status = 405)]
    InvalidInput(()),
    #[response(status = 409)]
    StateError(()),
}

#[post("/axis/<xy>/reference")]
pub fn post_axis_xy_reference(
    xy: ApiPostAxisXYReferenceAxis,
    decoder_ctrl: &State<DecoderCtrl>,
) -> Result<status::Accepted<()>, status::Custom<()>> {
    decoder_ctrl
        .try_reference_axis(xy.into())
        .map(|_| status::Accepted(None))
        .map_err(|_| status::Custom(Status { code: 409 }, ()))
}

#[derive(Debug, Deserialize)]
#[serde(tag = "direction", rename_all = "lowercase")]
pub enum ApiPostAxisZReferenceDirection {
    Endstop,
    Hotend,
}

#[post("/axis/z/reference", data = "<direction>")]
pub fn post_axis_z_reference(
    direction: JsonResult<ApiPostAxisZReferenceDirection>,
    decoder_ctrl: &State<DecoderCtrl>,
) -> ApiPostAxisReferenceResponse {
    let direction = json_ok_or!(direction, ApiPostAxisReferenceResponse::InvalidInput(()));
    match direction {
        ApiPostAxisZReferenceDirection::Endstop => match decoder_ctrl.try_reference_axis(Axis::Z) {
            Ok(_) => ApiPostAxisReferenceResponse::Accepted(()),
            Err(_) => ApiPostAxisReferenceResponse::StateError(()),
        },
        ApiPostAxisZReferenceDirection::Hotend => todo!(),
    }
}
