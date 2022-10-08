use rocket::{get, http::Status, post, response::status};

#[get("/gcode")]
pub fn get() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/start")]
pub fn post_start() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/stop")]
pub fn post_stop() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/continue")]
pub fn post_continue() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}

#[post("/gcode/pause")]
pub fn post_pause() -> status::Custom<&'static str> {
    status::Custom(Status::NotImplemented, "unimplemented")
}
