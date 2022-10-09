use rocket::{get, http::Status, put, response::status};

#[get("/heating/hotend/settings")]
pub fn get_hotend_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/heating/bed/settings")]
pub fn get_bed_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[get("/heating/chamber/settings")]
pub fn get_chamber_settings() -> status::Custom<()> {
    status::Custom(Status::NotImplemented, ())
}

#[put("/heating/hotend/settings")]
pub fn put_hotend_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[put("/heating/bed/settings")]
pub fn put_bed_settings() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[put("/heating/chamber/settings")]
pub fn put_chamber_settings() -> status::Custom<()> {
    status::Custom(Status::NotImplemented, ())
}
