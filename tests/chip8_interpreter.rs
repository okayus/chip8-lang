/// CHIP-8 ミニインタプリタ (テスト用)
///
/// コンパイラが生成するオペコードのみサポート。
/// 描画・タイマー・キー入力命令は NOP 扱い。

const MEM_SIZE: usize = 4096;
const PROGRAM_START: u16 = 0x200;
const MAX_CYCLES: usize = 10000;

pub struct Chip8 {
    mem: [u8; MEM_SIZE],
    v: [u8; 16],
    i: u16,
    pc: u16,
    stack: Vec<u16>,
}

impl Chip8 {
    pub fn new(program: &[u8]) -> Self {
        let mut mem = [0u8; MEM_SIZE];
        let start = PROGRAM_START as usize;
        let end = start + program.len();
        mem[start..end].copy_from_slice(program);
        Chip8 {
            mem,
            v: [0; 16],
            i: 0,
            pc: PROGRAM_START,
            stack: Vec::new(),
        }
    }

    /// プログラムを実行し、停止時の V0 を返す。
    /// ハング検出時は None。
    pub fn run_and_get_v0(&mut self) -> Option<u8> {
        for _ in 0..MAX_CYCLES {
            let hi = self.mem[self.pc as usize] as u16;
            let lo = self.mem[self.pc as usize + 1] as u16;
            let op = (hi << 8) | lo;

            let nnn = op & 0x0FFF;
            let x = ((op >> 8) & 0x0F) as usize;
            let y = ((op >> 4) & 0x0F) as usize;
            let kk = (op & 0xFF) as u8;
            let nibble = (op & 0x0F) as u8;

            match op & 0xF000 {
                0x0000 => match op {
                    0x00E0 => {
                        // CLS - nop
                        self.pc += 2;
                    }
                    0x00EE => {
                        // RET
                        if let Some(addr) = self.stack.pop() {
                            self.pc = addr;
                        } else {
                            // スタック空 = main 終了
                            return Some(self.v[0]);
                        }
                    }
                    _ => self.pc += 2, // unknown 0xxx - skip
                },
                0x1000 => {
                    // JP nnn
                    if nnn == self.pc {
                        // jump-to-self = halt
                        return Some(self.v[0]);
                    }
                    self.pc = nnn;
                }
                0x2000 => {
                    // CALL nnn
                    self.stack.push(self.pc + 2);
                    self.pc = nnn;
                }
                0x3000 => {
                    // SE Vx, kk
                    self.pc += if self.v[x] == kk { 4 } else { 2 };
                }
                0x4000 => {
                    // SNE Vx, kk
                    self.pc += if self.v[x] != kk { 4 } else { 2 };
                }
                0x5000 => {
                    // SE Vx, Vy
                    self.pc += if self.v[x] == self.v[y] { 4 } else { 2 };
                }
                0x6000 => {
                    // LD Vx, kk
                    self.v[x] = kk;
                    self.pc += 2;
                }
                0x7000 => {
                    // ADD Vx, kk
                    self.v[x] = self.v[x].wrapping_add(kk);
                    self.pc += 2;
                }
                0x8000 => {
                    match nibble {
                        0x0 => self.v[x] = self.v[y],
                        0x1 => self.v[x] |= self.v[y],
                        0x2 => self.v[x] &= self.v[y],
                        0x3 => self.v[x] ^= self.v[y],
                        0x4 => {
                            let sum = self.v[x] as u16 + self.v[y] as u16;
                            self.v[0xF] = if sum > 255 { 1 } else { 0 };
                            self.v[x] = sum as u8;
                        }
                        0x5 => {
                            self.v[0xF] = if self.v[x] >= self.v[y] { 1 } else { 0 };
                            self.v[x] = self.v[x].wrapping_sub(self.v[y]);
                        }
                        0x6 => {
                            self.v[0xF] = self.v[y] & 1;
                            self.v[x] = self.v[y] >> 1;
                        }
                        0x7 => {
                            self.v[0xF] = if self.v[y] >= self.v[x] { 1 } else { 0 };
                            self.v[x] = self.v[y].wrapping_sub(self.v[x]);
                        }
                        0xE => {
                            self.v[0xF] = (self.v[y] >> 7) & 1;
                            self.v[x] = self.v[y] << 1;
                        }
                        _ => {}
                    }
                    self.pc += 2;
                }
                0x9000 => {
                    // SNE Vx, Vy
                    self.pc += if self.v[x] != self.v[y] { 4 } else { 2 };
                }
                0xA000 => {
                    // LD I, nnn
                    self.i = nnn;
                    self.pc += 2;
                }
                0xB000 => {
                    // JP V0, nnn
                    self.pc = nnn + self.v[0] as u16;
                }
                0xC000 => {
                    // RND Vx, kk - テスト用に固定値
                    self.v[x] = 0x42 & kk;
                    self.pc += 2;
                }
                0xD000 => {
                    // DRW - nop (collision = 0)
                    self.v[0xF] = 0;
                    self.pc += 2;
                }
                0xE000 => {
                    match kk {
                        0x9E => {
                            // SKP - キー未押下として扱う
                            self.pc += 2;
                        }
                        0xA1 => {
                            // SKNP - キー未押下として扱う → スキップ
                            self.pc += 4;
                        }
                        _ => self.pc += 2,
                    }
                }
                0xF000 => {
                    match kk {
                        0x07 => {
                            // LD Vx, DT - タイマー = 0
                            self.v[x] = 0;
                            self.pc += 2;
                        }
                        0x0A => {
                            // LD Vx, K - テスト用に 0 を返す
                            self.v[x] = 0;
                            self.pc += 2;
                        }
                        0x15 | 0x18 => {
                            // LD DT/ST - nop
                            self.pc += 2;
                        }
                        0x1E => {
                            // ADD I, Vx
                            self.i += self.v[x] as u16;
                            self.pc += 2;
                        }
                        0x29 => {
                            // LD F, Vx - フォントアドレス
                            self.i = (self.v[x] as u16) * 5;
                            self.pc += 2;
                        }
                        0x33 => {
                            // BCD
                            let val = self.v[x];
                            self.mem[self.i as usize] = val / 100;
                            self.mem[self.i as usize + 1] = (val / 10) % 10;
                            self.mem[self.i as usize + 2] = val % 10;
                            self.pc += 2;
                        }
                        0x55 => {
                            // LD [I], V0..Vx
                            for r in 0..=x {
                                self.mem[self.i as usize + r] = self.v[r];
                            }
                            self.pc += 2;
                        }
                        0x65 => {
                            // LD V0..Vx, [I]
                            for r in 0..=x {
                                self.v[r] = self.mem[self.i as usize + r];
                            }
                            self.pc += 2;
                        }
                        _ => self.pc += 2,
                    }
                }
                _ => self.pc += 2,
            }
        }
        // サイクル上限到達 = ハング
        None
    }
}
