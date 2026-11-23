//! Code generation targets.

pub mod x86_64;

/// A machine the compiler can produce an executable for.
pub trait Target {
    fn name(&self) -> &'static str;
    fn executable(&self, m: &crate::ir::Module) -> Result<Vec<u8>, String>;
}

struct X86_64Linux;

impl Target for X86_64Linux {
    fn name(&self) -> &'static str {
        "x86-64 Linux"
    }

    fn executable(&self, _m: &crate::ir::Module) -> Result<Vec<u8>, String> {
        Err("code generation for x86-64 Linux is not finished".to_string())
    }
}

pub fn x86_64_linux() -> Box<dyn Target> {
    Box::new(X86_64Linux)
}

#[cfg(test)]
mod tests {
    use super::x86_64_linux;

    #[test]
    fn the_x86_64_linux_target_is_named() {
        assert_eq!(x86_64_linux().name(), "x86-64 Linux");
    }
}
