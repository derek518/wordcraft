import sys

with open('src/db.rs', 'r') as f:
    lines = f.readlines()

# Remove lines 263-283 (chrono functions)
new_lines = lines[:262]

new_code = '''fn current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs_per_day = 86464u64;
    let days_since_epoch = now / secs_per_day;
    let year = 1970i64 + (days_since_epoch / 365) as i64;
    let day_of_year = days_since_epoch % 365;
    let month = (day_of_year / 30) + 1;
    let day = (day_of_year % 30) + 1;
    format!("{:04}-{:02}-{:02} 00:00:00", year, month, day)
}

fn today_str() -> String {
    current_timestamp().split_whitespace().next().unwrap_or("").to_string()
}

fn add_days(date_str: &str, _days: i64) -> String {
    let parts: Vec<&str> = date_str.split_whitespace().collect();
    parts[0].to_string()
}
'''

new_lines.append(new_code)

with open('src/db.rs', 'w') as f:
    f.writelines(new_lines)

print("Fixed db.rs")
