pub fn print_logo() {
    const LOGO: &str = include_str!("../assets/geminis-labs-logo.txt");
    const GRAY: &str = "\x1b[38;2;180;180;180m";
    const WHITE: &str = "\x1b[97m";
    const RESET: &str = "\x1b[0m";

    println!();
    println!("\t\t{WHITE}GeminiLabs :: Alert Processor{RESET}");
    println!("{GRAY}────────────────────────────────────────────────────────────────{RESET}");
    println!();

    use std::io::Write;
    if let Err(e) = std::io::stdout().write_all(LOGO.as_bytes()) {
        tracing::warn!(error = %e, "failed to print startup logo");
    }

    println!();
    println!("{GRAY}────────────────────────────────────────────────────────────────{RESET}");
    println!("\t\t{GRAY}alert-processor • @geminislabs{RESET}");
    println!();
    println!();
}
