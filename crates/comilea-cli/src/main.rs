use comilea_core::Machine;

fn parse_cycles() -> Result<u64, String> {
    let mut args = std::env::args().skip(1);
    let mut cycles = 0_u64;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cycles" => {
                let value = args.next().ok_or("missing value for --cycles")?;
                cycles = value.parse::<u64>().map_err(|_| "invalid --cycles value")?;
            }
            "-h" | "--help" => {
                println!("Comilea headless M0 runner\n\nUsage: comilea-cli [--cycles N]");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(cycles)
}

fn main() {
    let cycles = match parse_cycles() {
        Ok(value) => value,
        Err(message) => {
            eprintln!("error: {message}");
            std::process::exit(2);
        }
    };

    let mut machine = Machine::new();
    machine.step_cycles(cycles);

    println!(
        "Comilea M0: cycles={} pc=${:04x} a=${:02x} x=${:02x} y=${:02x}",
        machine.cycles(),
        machine.cpu().pc,
        machine.cpu().a,
        machine.cpu().x,
        machine.cpu().y
    );
}
