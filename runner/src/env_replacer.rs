use regex::Regex;

pub fn env_replace(input: String, re: Regex) -> String {
    re.replace_all(&input, EnvReplacer).into_owned()
}

struct EnvReplacer;

impl regex::Replacer for EnvReplacer {
    fn replace_append(&mut self, caps: &regex::Captures, dst: &mut String) {
        let var_name = caps
            .get(1)
            .unwrap_or_else(|| {
                eprintln!("invalid env replace regex. 1 capture group must be included.");
                std::process::exit(1)
            })
            .as_str();

        let env_var = std::env::var(var_name).unwrap_or_else(|_| {
            log::info!(
                "cannot get enviroment variable \"{}\", replace with empty string",
                var_name,
            );
            String::new()
        });
        log::info!("replace {} by {}", var_name, env_var);

        dst.push_str(&env_var);
    }
}
