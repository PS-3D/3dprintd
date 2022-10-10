use super::{ApiPutSettingsResponse, JsonResult};
use rocket::{get, http::Status, put, response::status, serde::json::Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiGetHotendSettings {}

#[get("/heating/hotend/settings")]
pub fn get_hotend_settings() -> status::Custom<Json<ApiGetHotendSettings>> {
    status::Custom(Status::Ok, Json(ApiGetHotendSettings {}))
}

#[derive(Debug, Serialize)]
pub struct ApiGetBedSettings {}

#[get("/heating/bed/settings")]
pub fn get_bed_settings() -> status::Custom<Json<ApiGetBedSettings>> {
    status::Custom(Status::Ok, Json(ApiGetBedSettings {}))
}

#[get("/heating/chamber/settings")]
pub fn get_chamber_settings() -> status::Custom<()> {
    status::Custom(Status::NotImplemented, ())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiPutHotendSettings {}

#[put("/heating/hotend/settings", data = "<received_settings>")]
pub fn put_hotend_settings(
    received_settings: JsonResult<ApiPutHotendSettings>,
) -> ApiPutSettingsResponse {
    let _received_setings = match received_settings {
        Ok(s) => s,
        Err(_) => return ApiPutSettingsResponse::InvalidInput(()),
    };
    ApiPutSettingsResponse::Ok(())
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ApiPutBedSettings {}

#[put("/heating/bed/settings", data = "<received_settings>")]
pub fn put_bed_settings(
    received_settings: JsonResult<ApiPutBedSettings>,
) -> ApiPutSettingsResponse {
    let _received_setings = match received_settings {
        Ok(s) => s,
        Err(_) => return ApiPutSettingsResponse::InvalidInput(()),
    };
    ApiPutSettingsResponse::Ok(())
}

#[put("/heating/chamber/settings")]
pub fn put_chamber_settings() -> status::Custom<()> {
    status::Custom(Status::NotImplemented, ())
}
