use crate::comms::Axis;
use rocket::{get, http::Status, post, put, response::status};

#[get("/axis/position")]
pub fn get_position() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/axis/<axis_name>/position")]
pub fn get_axis_name_position(axis_name: Axis) -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/axis/<axis_name>/settings")]
pub fn get_axis_name_settings(axis_name: Axis) -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/axis/e/settings")]
pub fn get_e_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[put("/axis/<axis_name>/settings")]
pub fn put_axis_name_settings(axis_name: Axis) -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[put("/axis/e/settings")]
pub fn put_e_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/axis/<axis_name>/reference")]
pub fn post_axis_name_reference(axis_name: Axis) -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}
