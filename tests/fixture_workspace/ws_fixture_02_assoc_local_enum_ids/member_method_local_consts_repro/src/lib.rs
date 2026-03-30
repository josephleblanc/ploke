pub struct PyStructSequenceMeta;

impl PyStructSequenceMeta {
    pub fn class_name(&self) {
        const KEY: &str = "name";
        let _ = KEY;
    }

    pub fn module(&self) {
        const KEY: &str = "module";
        let _ = KEY;
    }

    fn data_type(&self) {
        const KEY: &str = "data";
        let _ = KEY;
    }
}

pub fn exercise_fixture() {
    let meta = PyStructSequenceMeta;
    meta.class_name();
    meta.module();
    meta.data_type();
}
