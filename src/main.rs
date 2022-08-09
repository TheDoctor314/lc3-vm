mod vm;

use vm::Vm;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn try_main() -> anyhow::Result<()> {
    let mut args = std::env::args();
    args.next();

    let file = match args.next() {
        Some(file) => file,
        None => {
            eprintln!("Usage: lc3-vm binary");
            std::process::exit(1)
        }
    };

    let mut vm = Vm::new(0x3000, vm::Flag::Zero as u16);
    vm.read_image(file)?;

    vm.run();

    Ok(())
}
