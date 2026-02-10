pub fn format_bytes(bytes: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes < KB {
        format!("{:.0} B", bytes)
    } else if bytes < MB {
        format!("{:.1} KB", bytes / KB)
    } else if bytes < GB {
        format!("{:.1} MB", bytes / MB)
    } else {
        format!("{:.1} GB", bytes / GB)
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_zero_bytes() {
        assert_eq!(format_bytes(0.0), "0 B");
    }

    #[test]
    fn formats_small_bytes_no_decimal() {
        assert_eq!(format_bytes(999.0), "999 B");
    }

    #[test]
    fn formats_1_kb_exact() {
        assert_eq!(format_bytes(1024.0), "1.0 KB");
    }

    #[test]
    fn formats_1_1_kb_example() {
        assert_eq!(format_bytes(1100.0), "1.1 KB");
    }

    #[test]
    fn formats_just_below_1_mb() {
        assert_eq!(format_bytes((1024.0 * 1024.0) - 1.0), "1024.0 KB");
    }

    #[test]
    fn formats_1_mb_exact() {
        assert_eq!(format_bytes(1024.0 * 1024.0), "1.0 MB");
    }

    #[test]
    fn formats_given_example_1310720_bytes() {
        assert_eq!(format_bytes(1_310_720.0), "1.2 MB");
    }

    #[test]
    fn rounds_correctly_to_one_decimal() {
        assert_eq!(format_bytes(1076.0), "1.1 KB");
    }

    #[test]
    fn formats_1_gb_exact() {
        assert_eq!(format_bytes(1024.0_f64.powi(3)), "1.0 GB");
    }

    #[test]
    fn clamps_to_gb_for_very_large_sizes() {
        assert_eq!(format_bytes(1024.0_f64.powi(4)), "1024.0 GB");
    }
}
