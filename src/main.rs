mod vm;

use std::{
    io::{self, stdin, Read},
    os::unix::prelude::AsRawFd,
};

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

    let _terminal = enable_raw_mode()?;

    // loop {
    //     match getch()? {
    //         b'q' => break,
    //         c => {
    //             println!("{c}: '{}'", c.escape_ascii());
    //         }
    //     }
    // }

    vm.run();

    Ok(())
}

fn getch() -> io::Result<u8> {
    let mut buf = [0u8; 1];
    let mut stdin = stdin();

    loop {
        if stdin.read(&mut buf)? != 0 {
            return Ok(buf[0]);
        }
    }
}

struct Terminal(termios::Termios);

impl Drop for Terminal {
    fn drop(&mut self) {
        use termios::*;

        let _ = tcsetattr(stdin().as_raw_fd(), TCSAFLUSH, &self.0);
    }
}

fn enable_raw_mode() -> anyhow::Result<Terminal> {
    use termios::*;
    let stdin = stdin();
    let mut termios = Termios::from_fd(stdin.as_raw_fd())?;

    let c_lflag = termios.c_lflag;
    termios.c_lflag &= !(ECHO | ICANON);

    tcsetattr(stdin.as_raw_fd(), TCSAFLUSH, &termios)?;

    termios.c_lflag = c_lflag;

    // this struct has now the original attributes of the terminal
    Ok(Terminal(termios))
}
