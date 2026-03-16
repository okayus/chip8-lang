use std::fs;
use std::path::Path;

/// ROM の最大サイズ (4KB - 0x200 = 3584 バイト)
const MAX_ROM_SIZE: usize = 4096 - 0x200;

/// Emitter のエラー
#[derive(Debug)]
pub enum EmitError {
    /// ROM サイズが CHIP-8 の制限を超えている
    RomTooLarge { size: usize, max: usize },
    /// ファイル書き込みに失敗
    IoError(std::io::Error),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmitError::RomTooLarge { size, max } => {
                write!(f, "ROM size {size} bytes exceeds maximum {max} bytes")
            }
            EmitError::IoError(e) => write!(f, "failed to write output file: {e}"),
        }
    }
}

/// バイトコードを `.ch8` ファイルとして書き出す
pub fn emit(bytes: &[u8], output_path: &Path) -> Result<(), EmitError> {
    if bytes.len() > MAX_ROM_SIZE {
        return Err(EmitError::RomTooLarge {
            size: bytes.len(),
            max: MAX_ROM_SIZE,
        });
    }

    fs::write(output_path, bytes).map_err(EmitError::IoError)?;

    Ok(())
}
