use std::fs;
use std::path::Path;

/// ROM の最大サイズ (4KB - 0x200 = 3584 バイト)
const MAX_ROM_SIZE: usize = 4096 - 0x200;

/// Emitter のエラー
#[derive(Debug)]
pub struct EmitError {
    pub message: String,
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// バイトコードを `.ch8` ファイルとして書き出す
pub fn emit(bytes: &[u8], output_path: &Path) -> Result<(), EmitError> {
    if bytes.len() > MAX_ROM_SIZE {
        return Err(EmitError {
            message: format!(
                "ROM size {} bytes exceeds maximum {} bytes",
                bytes.len(),
                MAX_ROM_SIZE
            ),
        });
    }

    fs::write(output_path, bytes).map_err(|e| EmitError {
        message: format!("failed to write output file: {e}"),
    })?;

    Ok(())
}
