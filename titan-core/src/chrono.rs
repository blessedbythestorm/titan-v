pub fn format_duration(seconds: &f64) -> String {
    let abs_seconds = seconds.abs();

    let (value, unit) = if abs_seconds < 1.0 {
        let millis = abs_seconds * 1_000.0;
        (millis, "ms")
    } else {
        (abs_seconds, "s")
    };

    format!("{:0>7.2} {}", value, unit)
}
