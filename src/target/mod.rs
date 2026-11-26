//! Code generation targets.

pub mod elf;
pub mod listing;
pub mod x86_64;

/// A machine the compiler can produce an executable for.
pub trait Target {
    fn name(&self) -> &'static str;
    fn executable(&self, m: &crate::ir::Module) -> Result<Vec<u8>, String>;
    fn listing(&self, m: &crate::ir::Module) -> Result<Vec<listing::Routine>, String>;
}

struct X86_64Linux;

impl Target for X86_64Linux {
    fn name(&self) -> &'static str {
        "x86-64 Linux"
    }

    fn executable(&self, m: &crate::ir::Module) -> Result<Vec<u8>, String> {
        x86_64::program::executable(m)
    }

    fn listing(&self, m: &crate::ir::Module) -> Result<Vec<listing::Routine>, String> {
        x86_64::listing::listing(&x86_64::program::assemble(m)?, m)
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
