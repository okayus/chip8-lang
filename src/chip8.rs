/// ユーザー割り当て可能レジスタ V0-VE (index 0-14)
///
/// コード生成がローカル変数やテンポラリに割り当てるレジスタ。
/// VF は CHIP-8 のフラグレジスタであり、コンパイラが変数に割り当てることはないため除外される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserRegister(u8);

impl UserRegister {
    pub fn new(index: u8) -> Self {
        debug_assert!(
            index <= 14,
            "UserRegister must be V0-VE (0-14), got {index}"
        );
        Self(index)
    }

    pub fn index(self) -> u8 {
        self.0
    }
}

/// CHIP-8 汎用レジスタ (V0-VF)
///
/// 命令のオペランドとして使用される。V0-VE はユーザー割り当て可能、VF はフラグレジスタ。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Register {
    User(UserRegister),
    Flag,
}

impl Register {
    pub const V0: Self = Self::User(UserRegister(0));
    pub const VF: Self = Self::Flag;

    pub fn index(self) -> u8 {
        match self {
            Self::User(u) => u.index(),
            Self::Flag => 0x0F,
        }
    }
}

impl From<UserRegister> for Register {
    fn from(ur: UserRegister) -> Self {
        Self::User(ur)
    }
}

/// CHIP-8 の 12bit メモリアドレス (0x000..=0xFFF)
///
/// CHIP-8 は 4KB のアドレス空間を持ち、ユーザープログラムは 0x200 から配置される。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Addr(u16);

impl Addr {
    pub const PROGRAM_START: Self = Self(0x200);

    pub fn new(raw: u16) -> Self {
        debug_assert!(raw <= 0xFFF, "CHIP-8 address must be 12-bit (0x000-0xFFF)");
        Self(raw)
    }

    pub fn raw(self) -> u16 {
        self.0
    }
}

/// バイトコード配列中のバイトオフセット
///
/// 前方参照のパッチやループの break 解決で、後から命令を書き換える位置を記録する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteOffset(pub usize);

/// スプライトの高さ (1-15 ピクセル行)
///
/// CHIP-8 のスプライトは幅 8px 固定、高さ 1-15px。
/// 各行は 1 バイトで表現されるため、高さ＝バイト数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpriteHeight(u8);

impl SpriteHeight {
    pub fn new(n: u8) -> Self {
        debug_assert!(
            (1..=15).contains(&n),
            "CHIP-8 sprite height must be 1-15, got {n}"
        );
        Self(n)
    }

    pub fn value(self) -> u8 {
        self.0
    }
}

/// CHIP-8 命令セット
///
/// 各バリアントが 1 つの CHIP-8 命令に対応し、必要なオペランドを型安全に保持する。
/// `encode()` で 2 バイトのビッグエンディアン表現に変換できる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    /// 00E0 - 画面クリア
    Cls,
    /// 00EE - サブルーチンからリターン
    Ret,
    /// 1NNN - アドレスにジャンプ
    Jp(Addr),
    /// 2NNN - サブルーチン呼び出し
    Call(Addr),
    /// 3XKK - Vx == kk なら次の命令をスキップ
    SeImm(Register, u8),
    /// 4XKK - Vx != kk なら次の命令をスキップ
    SneImm(Register, u8),
    /// 5XY0 - Vx == Vy なら次の命令をスキップ
    SeReg(Register, Register),
    /// 6XKK - Vx に即値をロード
    LdImm(Register, u8),
    /// 7XKK - Vx に即値を加算 (キャリーなし)
    AddImm(Register, u8),
    /// 8XY0 - Vx = Vy
    LdReg(Register, Register),
    /// 8XY1 - Vx |= Vy
    Or(Register, Register),
    /// 8XY2 - Vx &= Vy
    And(Register, Register),
    /// 8XY3 - Vx ^= Vy
    Xor(Register, Register),
    /// 8XY4 - Vx += Vy (VF = キャリー)
    Add(Register, Register),
    /// 8XY5 - Vx -= Vy (VF = NOT ボロー)
    Sub(Register, Register),
    /// 8XY7 - Vx = Vy - Vx (VF = NOT ボロー)
    Subn(Register, Register),
    /// 9XY0 - Vx != Vy なら次の命令をスキップ
    SneReg(Register, Register),
    /// ANNN - I レジスタにアドレスをセット
    LdI(Addr),
    /// CXKK - Vx = random AND kk
    Rnd(Register, u8),
    /// DXYN - (Vx, Vy) にスプライトを描画、高さ N
    Drw(Register, Register, SpriteHeight),
    /// EX9E - キー Vx が押されていれば次をスキップ
    Skp(Register),
    /// EXA1 - キー Vx が押されていなければ次をスキップ
    Sknp(Register),
    /// FX07 - Vx = ディレイタイマー
    LdVxDt(Register),
    /// FX0A - キー入力を待ち、Vx に格納
    LdVxK(Register),
    /// FX15 - ディレイタイマー = Vx
    LdDtVx(Register),
    /// FX18 - サウンドタイマー = Vx
    LdStVx(Register),
    /// FX1E - I += Vx
    AddI(Register),
    /// FX29 - I = 数字 Vx のフォントスプライトアドレス
    LdFVx(Register),
    /// FX33 - Vx の BCD 表現を I, I+1, I+2 に格納
    LdBVx(Register),
    /// FX65 - I から V0..Vx にメモリを読み込み
    LdVxI(Register),
    /// FX55 - V0..Vx をメモリ I に書き込み
    LdIVx(Register),
    /// 8XY6 - Vx = Vy >> 1, VF = LSB
    Shr(Register, Register),
    /// 8XYE - Vx = Vy << 1, VF = MSB
    Shl(Register, Register),
    /// BNNN - PC = V0 + NNN
    JpV0(Addr),
}

impl Opcode {
    /// 命令を 2 バイトのビッグエンディアン表現にエンコードする
    pub fn encode(self) -> [u8; 2] {
        match self {
            Opcode::Cls => [0x00, 0xE0],
            Opcode::Ret => [0x00, 0xEE],
            Opcode::Jp(addr) => {
                let nnn = addr.raw();
                [0x10 | ((nnn >> 8) as u8 & 0x0F), (nnn & 0xFF) as u8]
            }
            Opcode::Call(addr) => {
                let nnn = addr.raw();
                [0x20 | ((nnn >> 8) as u8 & 0x0F), (nnn & 0xFF) as u8]
            }
            Opcode::SeImm(vx, kk) => [0x30 | vx.index(), kk],
            Opcode::SneImm(vx, kk) => [0x40 | vx.index(), kk],
            Opcode::SeReg(vx, vy) => [0x50 | vx.index(), vy.index() << 4],
            Opcode::LdImm(vx, kk) => [0x60 | vx.index(), kk],
            Opcode::AddImm(vx, kk) => [0x70 | vx.index(), kk],
            Opcode::LdReg(vx, vy) => [0x80 | vx.index(), (vy.index() << 4)],
            Opcode::Or(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x01],
            Opcode::And(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x02],
            Opcode::Xor(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x03],
            Opcode::Add(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x04],
            Opcode::Sub(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x05],
            Opcode::Subn(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x07],
            Opcode::SneReg(vx, vy) => [0x90 | vx.index(), vy.index() << 4],
            Opcode::LdI(addr) => {
                let nnn = addr.raw();
                [0xA0 | ((nnn >> 8) as u8 & 0x0F), (nnn & 0xFF) as u8]
            }
            Opcode::Rnd(vx, kk) => [0xC0 | vx.index(), kk],
            Opcode::Drw(vx, vy, n) => [0xD0 | vx.index(), (vy.index() << 4) | n.value()],
            Opcode::Skp(vx) => [0xE0 | vx.index(), 0x9E],
            Opcode::Sknp(vx) => [0xE0 | vx.index(), 0xA1],
            Opcode::LdVxDt(vx) => [0xF0 | vx.index(), 0x07],
            Opcode::LdVxK(vx) => [0xF0 | vx.index(), 0x0A],
            Opcode::LdDtVx(vx) => [0xF0 | vx.index(), 0x15],
            Opcode::LdStVx(vx) => [0xF0 | vx.index(), 0x18],
            Opcode::AddI(vx) => [0xF0 | vx.index(), 0x1E],
            Opcode::LdFVx(vx) => [0xF0 | vx.index(), 0x29],
            Opcode::LdBVx(vx) => [0xF0 | vx.index(), 0x33],
            Opcode::LdVxI(vx) => [0xF0 | vx.index(), 0x65],
            Opcode::LdIVx(vx) => [0xF0 | vx.index(), 0x55],
            Opcode::Shr(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x06],
            Opcode::Shl(vx, vy) => [0x80 | vx.index(), (vy.index() << 4) | 0x0E],
            Opcode::JpV0(addr) => {
                let nnn = addr.raw();
                [0xB0 | ((nnn >> 8) as u8 & 0x0F), (nnn & 0xFF) as u8]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vx(n: u8) -> Register {
        UserRegister::new(n).into()
    }

    #[test]
    fn test_cls() {
        assert_eq!(Opcode::Cls.encode(), [0x00, 0xE0]);
    }

    #[test]
    fn test_ret() {
        assert_eq!(Opcode::Ret.encode(), [0x00, 0xEE]);
    }

    #[test]
    fn test_jp() {
        assert_eq!(Opcode::Jp(Addr::new(0x200)).encode(), [0x12, 0x00]);
        assert_eq!(Opcode::Jp(Addr::new(0xFFF)).encode(), [0x1F, 0xFF]);
    }

    #[test]
    fn test_call() {
        assert_eq!(Opcode::Call(Addr::new(0x200)).encode(), [0x22, 0x00]);
    }

    #[test]
    fn test_se_imm() {
        assert_eq!(Opcode::SeImm(vx(3), 0x42).encode(), [0x33, 0x42]);
    }

    #[test]
    fn test_sne_imm() {
        assert_eq!(Opcode::SneImm(vx(5), 0x00).encode(), [0x45, 0x00]);
    }

    #[test]
    fn test_se_reg() {
        assert_eq!(Opcode::SeReg(vx(1), vx(2)).encode(), [0x51, 0x20]);
    }

    #[test]
    fn test_ld_imm() {
        assert_eq!(Opcode::LdImm(Register::V0, 0xFF).encode(), [0x60, 0xFF]);
    }

    #[test]
    fn test_add_imm() {
        assert_eq!(Opcode::AddImm(vx(7), 0x01).encode(), [0x77, 0x01]);
    }

    #[test]
    fn test_alu_ops() {
        let v1 = vx(1);
        let v2 = vx(2);
        assert_eq!(Opcode::LdReg(v1, v2).encode(), [0x81, 0x20]);
        assert_eq!(Opcode::Or(v1, v2).encode(), [0x81, 0x21]);
        assert_eq!(Opcode::And(v1, v2).encode(), [0x81, 0x22]);
        assert_eq!(Opcode::Xor(v1, v2).encode(), [0x81, 0x23]);
        assert_eq!(Opcode::Add(v1, v2).encode(), [0x81, 0x24]);
        assert_eq!(Opcode::Sub(v1, v2).encode(), [0x81, 0x25]);
        assert_eq!(Opcode::Subn(v1, v2).encode(), [0x81, 0x27]);
    }

    #[test]
    fn test_sne_reg() {
        assert_eq!(Opcode::SneReg(vx(0xA), vx(0xB)).encode(), [0x9A, 0xB0]);
    }

    #[test]
    fn test_ld_i() {
        assert_eq!(Opcode::LdI(Addr::new(0x300)).encode(), [0xA3, 0x00]);
    }

    #[test]
    fn test_rnd() {
        assert_eq!(Opcode::Rnd(vx(4), 0xFF).encode(), [0xC4, 0xFF]);
    }

    #[test]
    fn test_drw() {
        assert_eq!(
            Opcode::Drw(vx(1), vx(2), SpriteHeight::new(5)).encode(),
            [0xD1, 0x25]
        );
    }

    #[test]
    fn test_key_ops() {
        let v3 = vx(3);
        assert_eq!(Opcode::Skp(v3).encode(), [0xE3, 0x9E]);
        assert_eq!(Opcode::Sknp(v3).encode(), [0xE3, 0xA1]);
    }

    #[test]
    fn test_fx_ops() {
        let v5 = vx(5);
        assert_eq!(Opcode::LdVxDt(v5).encode(), [0xF5, 0x07]);
        assert_eq!(Opcode::LdVxK(v5).encode(), [0xF5, 0x0A]);
        assert_eq!(Opcode::LdDtVx(v5).encode(), [0xF5, 0x15]);
        assert_eq!(Opcode::LdStVx(v5).encode(), [0xF5, 0x18]);
        assert_eq!(Opcode::AddI(v5).encode(), [0xF5, 0x1E]);
        assert_eq!(Opcode::LdFVx(v5).encode(), [0xF5, 0x29]);
        assert_eq!(Opcode::LdBVx(v5).encode(), [0xF5, 0x33]);
        assert_eq!(Opcode::LdVxI(v5).encode(), [0xF5, 0x65]);
    }

    #[test]
    fn test_ld_i_vx() {
        assert_eq!(Opcode::LdIVx(vx(5)).encode(), [0xF5, 0x55]);
    }

    #[test]
    fn test_shr() {
        assert_eq!(Opcode::Shr(vx(1), vx(2)).encode(), [0x81, 0x26]);
    }

    #[test]
    fn test_shl() {
        assert_eq!(Opcode::Shl(vx(1), vx(2)).encode(), [0x81, 0x2E]);
    }

    #[test]
    fn test_jp_v0() {
        assert_eq!(Opcode::JpV0(Addr::new(0x300)).encode(), [0xB3, 0x00]);
    }

    #[test]
    fn test_vf_register() {
        assert_eq!(Register::VF.index(), 0x0F);
        assert_eq!(Opcode::SeImm(Register::VF, 0x01).encode(), [0x3F, 0x01]);
    }
}
