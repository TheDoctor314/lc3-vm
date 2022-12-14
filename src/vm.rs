use anyhow::{bail, Result};
use log::info;
use std::{
    io::{stdout, Write},
    os::unix::prelude::AsRawFd,
    path::Path,
};

use crate::getch;

pub struct Vm {
    memory: Vec<u16>,
    pc: u16,
    reg: [u16; 8],
    psr: u16,
}

// addresses for the memory mapped regs
const KBSR: u16 = 0xFE00;
const KBDR: u16 = 0xFE02;
const DSR: u16 = 0xFE04;
const DDR: u16 = 0xFE06;

// traps
const GETC: u16 = 0x20;
const OUT: u16 = 0x21;
const PUTS: u16 = 0x22;
const IN: u16 = 0x23;
const PUTSP: u16 = 0x24;
const HALT: u16 = 0x25;

impl Vm {
    pub fn new(pc: u16, psr: u16) -> Self {
        Self {
            memory: vec![0; std::u16::MAX as usize],
            pc,
            reg: Default::default(),
            psr,
        }
    }

    pub fn read_image(&mut self, file: impl AsRef<Path>) -> Result<()> {
        let u16_len = std::mem::size_of::<u16>();
        let data = std::fs::read(file)?;

        let (origin, data) = data.split_at(u16_len);
        let origin = u16::from_be_bytes(origin.try_into().unwrap());

        self.pc = origin;

        let len = data.len() / u16_len;
        if len > u16::MAX as _ {
            bail!(
                "Input file too large - must not be greater than {} bytes",
                u16::MAX
            );
        }

        let dst = &mut self.memory[(origin as usize)..(origin as usize + len)];

        for (dst, src) in dst.iter_mut().zip(data.chunks(u16_len)) {
            *dst = u16::from_be_bytes(src.try_into().unwrap());
        }

        Ok(())
    }

    pub fn run(&mut self) {
        let mut running = true;

        while running {
            let inst = self.read_mem(self.pc);
            let op: Opcode = (inst >> 12).try_into().unwrap();

            info!("inst: {inst:#x} pc: {:#x}", self.pc);

            self.pc = self.pc.wrapping_add(1);

            match op {
                Opcode::Br => {
                    let nzp = inst >> 9 & 0b111;
                    let current_nzp = self.psr & 0b111;
                    let offset = sign_ext(inst, 9);

                    info!(
                        "Br current: {}, desired: {}, offset: {:#x}",
                        current_nzp, nzp, offset
                    );

                    if nzp & current_nzp != 0 {
                        self.pc = self.pc.wrapping_add(offset);
                    }
                }
                Opcode::Add => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    if inst & (1 << 5) != 0 {
                        let imm5 = sign_ext(inst, 5);

                        info!("Add r{dr}, r{sr1}, #{imm5}");

                        self.reg[dr] = self.reg[sr1].wrapping_add(imm5);
                    } else {
                        let sr2 = (inst & 0b111) as usize;

                        info!("Add r{dr}, r{sr1}, r{sr2}");

                        self.reg[dr] = self.reg[sr1].wrapping_add(self.reg[sr2]);
                    }

                    self.set_cc(dr);
                }
                Opcode::Ld => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let offset = sign_ext(inst, 9);

                    info!("Ld r{dr}, offset: {:#x}", offset);

                    self.reg[dr] = self.read_mem(self.pc.wrapping_add(offset));
                    self.set_cc(dr);
                }
                Opcode::St => {
                    let sr = (inst >> 9 & 0b111) as usize;
                    let offset = sign_ext(inst, 9);

                    info!("St r{sr} offset: {:#x}", offset);

                    self.write_mem(self.pc.wrapping_add(offset), self.reg[sr]);
                }
                Opcode::Jsr => {
                    let temp = self.pc;
                    self.pc = if inst & (1 << 11) != 0 {
                        let offset = sign_ext(inst, 11);

                        info!("Jsr offset: {:#x}", offset);

                        self.pc.wrapping_add(offset)
                    } else {
                        let br = (inst >> 6 & 0b111) as usize;
                        let br_val = self.reg[br];

                        info!("Jsr br_val: {}", br_val);
                        br_val
                    };

                    self.reg[7] = temp;
                }
                Opcode::And => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    if inst & (1 << 5) != 0 {
                        let imm5 = sign_ext(inst, 5);

                        info!("And r{dr}, r{sr1}, #{imm5}");

                        self.reg[dr] = self.reg[sr1] & imm5;
                    } else {
                        let sr2 = (inst & 0b111) as usize;

                        info!("And r{dr}, r{sr1}, r{sr2}");

                        self.reg[dr] = self.reg[sr1] & self.reg[sr2];
                    }

                    self.set_cc(dr);
                }
                Opcode::Ldr => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let br = (inst >> 6 & 0b111) as usize;
                    let offset = sign_ext(inst, 6);

                    info!("Ldr r{dr}, br: {br}, offset: {:#x}", offset);

                    let addr = self.reg[br].wrapping_add(offset);
                    self.reg[dr] = self.read_mem(addr);

                    self.set_cc(dr);
                }
                Opcode::Str => {
                    let sr = (inst >> 9 & 0b111) as usize;
                    let br = (inst >> 6 & 0b111) as usize;
                    let offset = sign_ext(inst, 6);

                    info!("Str r{sr}, br: {br}, offset: {:#x}", offset);

                    let addr = self.reg[br].wrapping_add(offset);
                    self.write_mem(addr, self.reg[sr]);
                }
                Opcode::Not => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    info!("Not r{dr}, r{sr1}");

                    self.reg[dr] = !self.reg[sr1];

                    self.set_cc(dr);
                }
                Opcode::Ldi => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let offset = sign_ext(inst, 9);
                    let addr = self.read_mem(self.pc.wrapping_add(offset));

                    info!("Ldi r{dr} offset: {:#x}", offset);

                    self.reg[dr] = self.read_mem(addr);
                    self.set_cc(dr);
                }
                Opcode::Sti => {
                    let sr = (inst >> 9 & 0b111) as usize;
                    let offset = sign_ext(inst, 9);

                    info!("Sti r{sr} offset: {:#x}", offset);

                    let addr = self.read_mem(self.pc.wrapping_add(offset));

                    self.write_mem(addr, self.reg[sr]);
                }
                Opcode::Jmp => {
                    let br = (inst >> 6 & 0b111) as usize;

                    info!("Jmp {br}");

                    self.pc = self.reg[br];
                }
                Opcode::Lea => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let offset = sign_ext(inst, 9);

                    info!("Lea r{dr} offset: {:#x}", offset);

                    self.reg[dr] = self.pc.wrapping_add(offset);
                    self.set_cc(dr);
                }
                Opcode::Trap => {
                    // implement traps in assembly or rust?
                    self.reg[7] = self.pc;

                    let trap = inst & 0xFF;
                    info!("Trap {trap}");

                    match trap {
                        GETC => {
                            self.reg[0] = getch().unwrap_or_default() as u16;
                            self.set_cc(0);
                        }
                        OUT => {
                            let byte = self.reg[0] as u8;
                            let _ = stdout().write(&[byte]).unwrap();
                        }
                        PUTS => {
                            let addr = self.reg[0] as usize;
                            let slice = &self.memory[addr..];
                            let end = slice.iter().position(|w| *w == 0x0000).unwrap_or_default();
                            let slice_to_print = &slice[..end];

                            let mut stdout = stdout().lock();

                            for &word in slice_to_print {
                                let _ = stdout.write(&[word as u8]).unwrap();
                            }

                            stdout.flush().unwrap();
                        }
                        IN => {
                            let mut stdout = stdout().lock();
                            write!(stdout, "Enter a character: ").unwrap();
                            stdout.flush().unwrap();

                            let ch = getch().unwrap_or_default();
                            let _ = stdout.write(&[ch]).unwrap();
                        }
                        PUTSP => {
                            let addr = self.reg[0] as usize;
                            let slice = &self.memory[addr..];

                            let mut stdout = stdout().lock();

                            for &word in slice {
                                let bytes = u16::to_le_bytes(word);
                                if bytes[1] != 0 {
                                    let _ = stdout.write(&bytes).unwrap();
                                } else {
                                    let _ = stdout.write(&bytes[..1]).unwrap();
                                }
                            }

                            stdout.flush().unwrap();
                        }
                        HALT => {
                            println!("HALT");
                            running = false;
                        }
                        _ => unimplemented!("Bad trap"),
                    }
                }
                Opcode::Rti | Opcode::Reserved => unimplemented!("Bad opcode: {op:?}"),
            }
        }
    }

    fn read_mem(&self, addr: u16) -> u16 {
        match addr {
            KBSR => {
                if is_ready_to_read() {
                    0x80
                } else {
                    0
                }
            }
            KBDR => {
                if self.read_mem(KBSR) != 0 {
                    getch().unwrap_or_default() as u16
                } else {
                    0
                }
            }
            DSR => 0x80,
            DDR => 0,
            _ => self.memory[addr as usize],
        }
    }

    fn write_mem(&mut self, addr: u16, val: u16) {
        match addr {
            // do nothing
            KBSR | KBDR | DSR => (),
            DDR => {
                let mut stdout = stdout().lock();
                let _ = stdout.write(&[val as u8]).unwrap();
                stdout.flush().unwrap();
            }
            _ => self.memory[addr as usize] = val,
        }
    }

    fn set_cc(&mut self, r: usize) {
        let reg = self.reg[r];
        self.psr = if reg == 0 {
            Flag::Zero
        } else if reg & (1 << 15) != 0 {
            Flag::Neg
        } else {
            Flag::Pos
        } as u16;
    }
}

const fn sign_ext(mut val: u16, bits: u16) -> u16 {
    val &= (1 << bits) - 1;

    if (val >> (bits - 1) & 1) != 0 {
        val |= 0xFFFF << bits;
    }

    val
}

fn is_ready_to_read() -> bool {
    use nix::sys::{
        select::*,
        time::{TimeVal, TimeValLike},
    };

    let mut read_fds = FdSet::default();
    read_fds.insert(std::io::stdin().as_raw_fd());

    let mut timeout: TimeVal = TimeValLike::zero();

    select(1, &mut read_fds, None, None, &mut timeout).is_ok()
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
enum Opcode {
    Br = 0b0000,
    Add = 0b0001,
    Ld = 0b0010,
    St = 0b0011,
    Jsr = 0b0100,
    And = 0b0101,
    Ldr = 0b0110,
    Str = 0b0111,
    Rti = 0b1000,
    Not = 0b1001,
    Ldi = 0b1010,
    Sti = 0b1011,
    Jmp = 0b1100,
    Reserved = 0b1101,
    Lea = 0b1110,
    Trap = 0b1111,
}

#[derive(Debug)]
struct OpcodeConvertErr;
impl TryFrom<u16> for Opcode {
    type Error = OpcodeConvertErr;
    fn try_from(val: u16) -> Result<Self, Self::Error> {
        if val > Opcode::Trap as u16 {
            return Err(OpcodeConvertErr);
        }

        Ok(unsafe { std::mem::transmute(val as u8) })
    }
}

pub enum Flag {
    Pos = 1,
    Zero = 2,
    Neg = 4,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_ext() {
        assert_eq!(sign_ext(0b10011, 5), 0xfff3);
        assert_eq!(sign_ext(0x30, 5), 0xfff0);
    }
}
