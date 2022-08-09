use anyhow::{bail, Result};
use std::{mem::size_of, path::Path};

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
        let data = std::fs::read(file)?;

        let (origin, data) = data.split_at(size_of::<u16>());
        let origin = u16::from_be_bytes(origin.try_into().unwrap());

        self.pc = origin;

        let len = data.len() / size_of::<u16>();
        if len > u16::MAX as _ {
            bail!(
                "Input file too large - must not be greater than {} bytes",
                u16::MAX
            );
        }

        let dst = &mut self.memory[(origin as usize)..(origin as usize + len)];

        for (dst, src) in dst.iter_mut().zip(data.chunks(size_of::<u16>())) {
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
                Opcode::Br => todo!(),
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
                Opcode::Ld => todo!(),
                Opcode::St => todo!(),
                Opcode::Jsr => todo!(),
                Opcode::And => todo!(),
                Opcode::Ldr => todo!(),
                Opcode::Str => todo!(),
                Opcode::Rti => todo!(),
                Opcode::Not => todo!(),
                Opcode::Ldi => todo!(),
                Opcode::Sti => todo!(),
                Opcode::Ret => todo!(),
                Opcode::Reserved => todo!(),
                Opcode::Lea => todo!(),
                Opcode::Trap => todo!(),
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

fn sign_ext(mut val: u16, bits: u16) -> u16 {
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

enum Opcode {
    Br = 0,
    Add,
    Ld,
    St,
    Jsr,
    And,
    Ldr,
    Str,
    Rti,
    Not,
    Ldi,
    Sti,
    Ret,
    Reserved,
    Lea,
    Trap,
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
