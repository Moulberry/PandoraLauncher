use self::super::ParameterBundle;
use self::super::windres::*;
use std::path::{PathBuf, MAIN_SEPARATOR};
use std::borrow::Cow;
use std::ffi::OsStr;
use std::{env, mem};


#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceCompiler {
    windres_target: Cow<'static, OsStr>,
    compiler: Result<Compiler, Cow<'static, str>>,
}


impl ResourceCompiler {
    #[inline(always)]
    pub fn new() -> ResourceCompiler {
        // Under some msys2 environments, $MINGW_CHOST has the correct target for
        // GNU windres or llvm-windres (clang32, clang64, or clangarm64)
        let target = env::var_os("TARGET").expect("No TARGET env var");
        let compiler = Compiler::choose(&target);
        ResourceCompiler {
            windres_target: match compiler.as_ref().map(|c| c.tp) {
                Ok(CompilerType::WindRes) => {
                    env::var_os("MINGW_CHOST").map(Cow::Owned).unwrap_or_else(|| {
                        OsStr::new(match target.as_encoded_bytes() {
                                [b'x', b'8', b'6', b'_', b'6', b'4', ..] => "pe-x86-64", // "x86_64"
                                [b'a', b'a', b'r', b'c', b'h', b'6', b'4', ..] => "pe-aarch64-little", // "aarch64"
                                // windres has "pe-aarch64-little" in the strings but doesn't actually accept it on my machine,
                                // llvm-windres only has i686 and amd64; still unported
                                _ => "pe-i386",
                            })
                            .into()
                    })
                }
                _ => OsStr::new("").into(),
            },
            compiler: compiler,
        }
    }

    #[inline(always)]
    pub fn is_supported(&mut self) -> Option<Cow<'static, str>> {
        self.compiler.as_mut().err().map(|e| mem::replace(e, "".into()))
    }

    pub fn compile_resource<Ms: AsRef<OsStr>, Mi: IntoIterator<Item = Ms>, Is: AsRef<OsStr>, Ii: IntoIterator<Item = Is>>(
        self, out_dir: &str, prefix: &str, resource: &str, parameters: ParameterBundle<Ms, Mi, Is, Ii>)
        -> Result<String, Cow<'static, str>> {
        let compiler = self.compiler.expect("Not supported but we got to compile_resource()?");
        compiler.compile(out_dir,
                         prefix,
                         format!("{}{}lib{}.a", out_dir, MAIN_SEPARATOR, prefix),
                         resource,
                         parameters,
                         "-fo",
                         "-C",
                         "-no-preprocess",
                         |c| {
                             c.arg("--target")
                                 .arg(self.windres_target)
                                 .args(&["-c", "65001"]) // UTF-8, cf. https://github.com/nabijaczleweli/rust-embed-resource/pull/73
                         })
    }
}

impl Compiler {
    fn choose(target: &OsStr) -> Result<Compiler, Cow<'static, str>> {
        match target.as_encoded_bytes() {
            // "aarch64".."gnullvm"
            // https://github.com/llvm/llvm-project/issues/125371
            [b'a', b'a', b'r', b'c', b'h', b'6', b'4', .., b'g', b'n', b'u', b'l', b'l', b'v', b'm'] => Compiler::llvm_rc("llvm-rc".into()),

            _ => Compiler::windres("windres".into()),
        }
    }
}

pub fn find_windows_sdk_tool_impl(_: &str) -> Option<PathBuf> {
    None
}
