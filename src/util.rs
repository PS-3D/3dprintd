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

// needed since anyhow::ensure makes everything into an anyhow::Error
macro_rules! ensure_own {
    ($condition:expr, $err:expr) => {{
        if !($condition) {
            return Err($err.into());
        }
    }};
}

// needed since anyhow::bail makes everything into an anyhow::Error
macro_rules! bail_own {
    ($err:expr) => {{
        return Err($err.into());
    }};
}

pub(crate) use {bail_own, ensure_own, send_err};
