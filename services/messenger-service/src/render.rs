use std::collections::HashMap;

/// Template rendering error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    MissingVariable(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::MissingVariable(name) => write!(f, "missing template variable: {name}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Replace `{{name}}` placeholders; an unknown name is an error, never blanked.
pub fn render(template: &str, vars: &HashMap<String, String>) -> Result<String, RenderError> {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;

    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("}}") {
            Some(end) => {
                let key = after[..end].trim();
                match vars.get(key) {
                    Some(value) => out.push_str(value),
                    None => return Err(RenderError::MissingVariable(key.to_string())),
                }
                rest = &after[end + 2..];
            }
            None => {
                // No closing braces; treat the remainder literally.
                out.push_str(&rest[start..]);
                return Ok(out);
            }
        }
    }

    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("username".to_string(), "alice".to_string());
        m.insert("otp".to_string(), "1234".to_string());
        m
    }

    #[test]
    fn substitutes_placeholders() {
        let out = render("Hi {{username}}, code {{ otp }}.", &vars()).unwrap();
        assert_eq!(out, "Hi alice, code 1234.");
    }

    #[test]
    fn leaves_plain_text_alone() {
        assert_eq!(
            render("no placeholders", &vars()).unwrap(),
            "no placeholders"
        );
    }

    #[test]
    fn missing_variable_errors() {
        assert_eq!(
            render("Hi {{nope}}", &vars()),
            Err(RenderError::MissingVariable("nope".to_string()))
        );
    }
}
