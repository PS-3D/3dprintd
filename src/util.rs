macro_rules! send_err {
    ($result:expr, $err_channel:ident) => {{
        match $result {
            Ok(r) => r,
            Err(e) => $err_channel
                .send(crate::comms::ControlComms::Msg(e))
                .unwrap(),
        }
    }};
}

pub(crate) use send_err;
