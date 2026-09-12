// Synthetic stdio provider for lifecycle tests only. No authentication or network.
use std::{
    fs::OpenOptions,
    io::{self, BufRead, Write},
};

fn main() {
    let dir = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned();
    let mut pids = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("provider-pids"))
        .unwrap();
    writeln!(pids, "{}", std::process::id()).unwrap();
    drop(pids);
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let Some(id) = line
            .split("\"id\":")
            .nth(1)
            .map(|s| {
                s.trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
            })
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        if line.contains("account/usage/read") {
            println!(
                "{{\"id\":{id},\"error\":{{\"code\":500,\"message\":\"SYNTHETIC_SECRET_TOKEN\"}}}}"
            );
        } else {
            let result = if line.contains("account/rateLimits/read") {
                r#"{"rateLimits":{"primary":{"usedPercent":20,"windowDurationMins":10080,"resetsAt":1900000000}}}"#
            } else if line.contains("account/read") {
                r#"{"account":{"email":"a@example.invalid","planType":"pro"}}"#
            } else {
                "{}"
            };
            println!("{{\"id\":{id},\"result\":{result}}}");
        }
        io::stdout().flush().unwrap();
    }
}
