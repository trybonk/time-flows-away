use std::{ffi::OsString, path::PathBuf};
#[derive(Clone)]
pub struct Options {
    pub preview: bool,
    pub settings: bool,
    pub worker: bool,
    pub opengl: bool,
    pub config: PathBuf,
}
impl Options {
    pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut result = Self {
            preview: false,
            settings: false,
            worker: false,
            opengl: cfg!(windows),
            config: time_flows_away_native::config::default_path(),
        };
        let mut renderer = None;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--preview") => result.preview = true,
                Some("--settings") => result.settings = true,
                Some("--wallpaper-worker") => result.worker = true,
                Some("--opengl" | "--wgpu") => {
                    let glow = arg == "--opengl";
                    if renderer.is_some_and(|previous| previous != glow) {
                        return Err("Choose only one renderer: --opengl or --wgpu".into());
                    }
                    renderer = Some(glow);
                    result.opengl = glow;
                }
                Some("--config") => {
                    result.config = PathBuf::from(
                        args.next()
                            .filter(|value| {
                                !value.is_empty() && !value.to_string_lossy().starts_with("--")
                            })
                            .ok_or("--config requires a path")?,
                    )
                }
                _ => return Err(format!("Unknown argument: {}", arg.to_string_lossy())),
            }
        }
        if result.settings && (result.preview || result.worker) {
            return Err("--settings cannot be combined with wallpaper or preview mode".into());
        }
        Ok(result)
    }
    #[cfg(any(windows, test))]
    pub fn child_args(&self) -> Vec<OsString> {
        vec![
            if self.opengl { "--opengl" } else { "--wgpu" }.into(),
            "--config".into(),
            self.config.clone().into_os_string(),
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(OsString::from))
    }
    #[test]
    fn invalid_combinations_and_missing_paths_fail() {
        for args in [
            &["--opengl", "--wgpu"][..],
            &["--config"],
            &["--config", "--preview"],
            &["--settings", "--preview"],
            &["--unknown"],
        ] {
            assert!(parse(args).is_err());
        }
    }
    #[test]
    fn renderer_and_custom_path_survive_child_launch() {
        let original = parse(&[
            "--settings",
            "--opengl",
            "--config",
            "settings with spaces.toml",
        ])
        .unwrap();
        let child = Options::parse(original.child_args()).unwrap();
        assert!(child.opengl);
        assert_eq!(child.config, original.config);
        assert!(!parse(&["--wgpu"]).unwrap().opengl);
    }
}
