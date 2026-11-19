// INFO: Primitive Logger. Stores logs in a file at ~/.local/share/ymp/log.txt

use std::io::Write;

pub fn log(message: &str, file_name: &str, line_number: u32) {
    let mut log_path = String::new();
    match dirs::data_local_dir() {
        Some(mut path) => {
            path.push("ymp");
            path.push("log.txt");
            log_path = path.to_string_lossy().into_owned();
        }
        None => {}
    }

    let message_string =
        String::from("") + file_name + ":" + &line_number.to_string() + ": " + message + "\n";
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .write(true)
        .open(&log_path)
    {
        Ok(file) => {
            m_write_to_log_file(message_string, &file);
        }
        Err(e) => eprintln!("{}", e),
    };
}

fn m_write_to_log_file(message: String, mut file: &std::fs::File) {
    if let Err(e) = file.write_all(message.as_bytes()) {
        eprintln!("{}", e);
    };
}
