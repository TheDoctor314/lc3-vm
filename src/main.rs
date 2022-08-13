mod vm;

use std::{
    io::{self, stdin, Read},
    os::unix::prelude::AsRawFd,
};

use anyhow::Result;
use nix::sys::termios;
use vm::Vm;

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn try_main() -> Result<()> {
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

        tcsetattr(stdin().as_raw_fd(), SetArg::TCSAFLUSH, &self.0).unwrap();
    }
}

fn enable_raw_mode() -> Result<Terminal> {
    use termios::*;

    let stdin = stdin().as_raw_fd();
    let mut termios = tcgetattr(stdin)?;

    let local_flags = termios.local_flags;

    let flags_to_remove = LocalFlags::ICANON | LocalFlags::ECHO;
    termios.local_flags &= flags_to_remove.complement();

    tcsetattr(stdin, SetArg::TCSAFLUSH, &termios)?;

    termios.local_flags = local_flags;

    // this struct has now the original attributes of the terminal
    Ok(Terminal(termios))
}
