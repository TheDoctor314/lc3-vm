use anyhow::{bail, Result};
use std::path::Path;

pub struct Vm {
    memory: Vec<u16>,
    pc: u16,
    reg: [u16; 8],
    psr: u16,
}

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
            self.pc += 1;

            let op: Opcode = (inst >> 12).try_into().unwrap();

            match op {
                Opcode::Br => {
                    let nzp = inst >> 9 & 0b111;
                    if nzp & (self.psr & 0b111) != 0 {
                        self.pc += sign_ext(inst, 9);
                    }
                }
                Opcode::Add => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    if inst & (1 << 5) != 0 {
                        let imm5 = sign_ext(inst, 5);
                        self.reg[dr] = self.reg[sr1] + imm5;
                    } else {
                        let sr2 = (inst & 0b111) as usize;
                        self.reg[dr] = self.reg[sr1] + self.reg[sr2];
                    }

                    self.set_cc(dr);
                }
                Opcode::Ld => {
                    let dr = (inst >> 9 & 0b111) as usize;

                    self.reg[dr] = self.read_mem(self.pc + sign_ext(inst, 9));
                    self.set_cc(dr);
                }
                Opcode::St => {
                    let sr = (inst >> 9 & 0b111) as usize;

                    self.write_mem(self.pc + sign_ext(inst, 9), self.reg[sr]);
                }
                Opcode::Jsr => {
                    let temp = self.pc;
                    self.pc = if inst & (1 << 11) != 0 {
                        self.pc + sign_ext(inst, 11)
                    } else {
                        inst >> 6 & 0b111
                    };

                    self.reg[7] = temp;
                }
                Opcode::And => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    if inst & (1 << 5) != 0 {
                        let imm5 = sign_ext(inst, 5);
                        self.reg[dr] = self.reg[sr1] & imm5;
                    } else {
                        let sr2 = (inst & 0b111) as usize;
                        self.reg[dr] = self.reg[sr1] & self.reg[sr2];
                    }

                    self.set_cc(dr);
                }
                Opcode::Ldr => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let br = (inst >> 6 & 0b111) as usize;

                    let addr = self.reg[br] + sign_ext(inst, 6);
                    self.reg[dr] = self.read_mem(addr);

                    self.set_cc(dr);
                }
                Opcode::Str => {
                    let sr = (inst >> 9 & 0b111) as usize;
                    let br = (inst >> 6 & 0b111) as usize;

                    let addr = self.reg[br] + sign_ext(inst, 6);
                    self.write_mem(addr, self.reg[sr]);
                }
                Opcode::Not => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let sr1 = (inst >> 6 & 0b111) as usize;

                    self.reg[dr] = !self.reg[sr1];

                    self.set_cc(dr);
                }
                Opcode::Ldi => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let addr = self.read_mem(self.pc + sign_ext(inst, 9));

                    self.reg[dr] = self.read_mem(addr);
                    self.set_cc(dr);
                }
                Opcode::Sti => {
                    let sr = (inst >> 9 & 0b111) as usize;
                    let addr = self.read_mem(self.pc + sign_ext(inst, 9));

                    self.write_mem(addr, self.reg[sr]);
                }
                Opcode::Jmp => {
                    let br = (inst >> 6 & 0b111) as usize;
                    self.pc = self.reg[br];
                }
                Opcode::Lea => {
                    let dr = (inst >> 9 & 0b111) as usize;
                    let addr = self.pc + sign_ext(inst, 9);

                    self.reg[dr] = addr;
                    self.set_cc(dr);
                }
                Opcode::Trap => {
                    // implement traps in assembly or rust?
                    todo!()
                }
                Opcode::Rti | Opcode::Reserved => unimplemented!("Bad opcode"),
            }
        }
    }

    fn read_mem(&self, addr: u16) -> u16 {
        self.memory[addr as usize]
    }

    fn write_mem(&mut self, addr: u16, val: u16) {
        self.memory[addr as usize] = val;
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
        val |= 0xFF << bits;
    }

    val
}

impl Default for Vm {
    fn default() -> Self {
        Self::new(0, 0)
    }
}

#[allow(dead_code)]
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
