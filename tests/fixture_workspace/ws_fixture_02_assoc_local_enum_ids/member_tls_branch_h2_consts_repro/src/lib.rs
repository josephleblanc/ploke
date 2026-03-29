pub enum Connector {
    Openssl,
    Rustls,
}

impl Connector {
    pub fn into_connection_io(self) {
        match self {
            Connector::Openssl => {
                const H2: &[u8] = b"h2";
                struct Local;
                impl Local {
                    fn touch() -> usize {
                        0
                    }
                }
                let _ = H2;
                let _ = Local::touch();
            }
            Connector::Rustls => {
                const H2: &[u8] = b"h2";
                struct Local;
                impl Local {
                    fn touch() -> usize {
                        0
                    }
                }
                let _ = H2;
                let _ = Local::touch();
            }
        }
    }
}

pub fn exercise_fixture() {
    Connector::Openssl.into_connection_io();
    Connector::Rustls.into_connection_io();
}
