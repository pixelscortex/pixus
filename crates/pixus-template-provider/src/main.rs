use std::{
    env,
    io::{self, Read},
    process::ExitCode,
};

use anyhow::{Context, Result, bail};
use proc_macro2::TokenStream;

const USAGE: &str = "Usage: pixus-template-provider project-body <macro-name>";

fn project_body(macro_name: &str, source: &str) -> Result<String> {
    if macro_name != "html" {
        bail!("unsupported macro name `{macro_name}` (only `html` is supported)");
    }

    let input: TokenStream = source
        .parse()
        .map_err(|error| anyhow::anyhow!("failed to parse macro body as tokens: {error}"))?;

    pixus_core::html_to_rsx_body(input)
        .context("failed to project html body")
        .map(|tokens| tokens.to_string())
}

fn run_project_body(macro_name: &str) -> Result<()> {
    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .context("failed to read macro body from stdin")?;
    println!("{}", project_body(macro_name, &source)?);
    Ok(())
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next();
    let macro_name = args.next();
    let trailing = args.next();

    match (command.as_deref(), macro_name, trailing) {
        (Some("project-body"), Some(macro_name), None) => run_project_body(&macro_name),
        (Some("--help" | "-h"), None, None) => {
            println!("{USAGE}");
            Ok(())
        }
        (Some("--version" | "-V"), None, None) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => bail!(USAGE),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("pixus-template-provider: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::project_body;

    #[test]
    fn projects_supported_html_body() {
        let projected = project_body("html", "<div class=\"greeting\">Hello</div>").unwrap();
        assert_eq!(projected, "div { class : \"greeting\" , \"Hello\" }");
    }

    #[test]
    fn rejects_unsupported_macro_name() {
        let error = project_body("rsx", "<div />").unwrap_err();
        assert!(error.to_string().contains("only `html` is supported"));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(project_body("html", "<div>").is_err());
    }
}
