use chrono::Local;

pub fn local_timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}
