use nanotec_stepper_driver::RotationDirection;

pub struct AxisMovement {
    pub distance: i32,
    pub min_frequency: u32,
    pub max_frequency: u32,
    // accel and decel are in hz/s
    pub acceleration: u32,
    pub deceleration: u32,
    pub acceleration_jerk: u32,
    pub deceleration_jerk: u32,
}

pub struct ExtruderMovement {
    pub direction: RotationDirection,
    pub distance: u32,
    pub min_frequency: u32,
    pub max_frequency: u32,
    // accel and decel are in hz/s
    pub acceleration: u32,
    pub deceleration: u32,
    pub acceleration_jerk: u32,
    pub deceleration_jerk: u32,
}

pub struct Movement {
    pub x: AxisMovement,
    pub y: AxisMovement,
    pub z: AxisMovement,
    pub e: ExtruderMovement,
}

pub enum MotorControl {
    MoveAll(Movement),
    ReferenceAll,
    Exit,
}

pub enum EStop {
    EStop,
    Exit,
}
