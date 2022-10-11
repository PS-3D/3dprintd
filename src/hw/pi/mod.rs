use revpi::revpi_from_json;

revpi_from_json!(RevPi, "config.rsc");

pub struct Pi(RevPi);

impl Pi {}
